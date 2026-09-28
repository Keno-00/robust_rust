//! Read-only APB player coordinates for the verified executable build.
//! The EchoDriver transport only reads; mouse output remains in the runner's
//! existing Arduino path.

use std::collections::HashSet;
use std::ffi::c_void;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use sha2::{Digest, Sha256};
use windows::core::{w, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, RECT};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetClientRect, GetForegroundWindow, GetWindowThreadProcessId,
};

use crate::detection::Target;
use crate::memory_projection::CameraProjection;
use crate::memory_reader::MemoryReader;

const EXPECTED_SHA256: &str = "3BECA02821F50A7B8FEF3E16923FA3FCA0D0B0DD49CC9622520EA500001E1640";
const TABLE_RVA: u64 = 0x3CFAC90;
const TABLE_ENTRIES: usize = 32768;
const CAMERA_CLASS: u32 = 16920;
const CONTROLLER_CLASS: u32 = 4159;
const LOCAL_LINK_CLASS: u32 = 6683;
const POSITION_CLASS: u32 = 4741;
const OWNER_CLASS: u32 = 4155;

#[repr(C)]
#[derive(Default)]
struct ProcessBasicInformation {
    reserved1: *mut c_void,
    peb: *mut c_void,
    reserved2: [*mut c_void; 2],
    unique_pid: usize,
    reserved3: *mut c_void,
}

#[link(name = "ntdll")]
extern "system" {
    fn NtQueryInformationProcess(
        process: HANDLE,
        information_class: u32,
        information: *mut ProcessBasicInformation,
        information_length: u32,
        return_length: *mut u32,
    ) -> i32;
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Discovery {
    pub pid: u32,
    pub base: u64,
    pub camera: u64,
    pub local_controller: u64,
    pub local_position: u64,
    pub local_link: u64,
    pub actors: Vec<u64>,
    pub readable_headers: usize,
    pub failed_headers: usize,
    pub skipped_owners: usize,
}

pub struct ApbMemorySource {
    reader: MemoryReader,
    discovery: Discovery,
    window: windows::Win32::Foundation::HWND,
    warned_stale: HashSet<u64>,
}

fn parse_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn parse_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn parse_position(bytes: &[u8], offset: usize) -> Result<[f32; 3], String> {
    let position = std::array::from_fn(|axis| {
        f32::from_le_bytes(
            bytes[offset + axis * 4..offset + axis * 4 + 4]
                .try_into()
                .unwrap(),
        )
    });
    if !position
        .iter()
        .all(|value| value.is_finite() && value.abs() < 1_000_000.0)
    {
        return Err("APB player position is invalid or nonfinite.".to_string());
    }
    Ok(position)
}

fn read_u64(reader: &MemoryReader, pid: u32, address: u64) -> Result<u64, String> {
    let mut bytes = [0u8; 8];
    reader.read_exact(pid, address, &mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_class(reader: &MemoryReader, pid: u32, address: u64) -> Result<u32, String> {
    let mut bytes = [0u8; 4];
    reader.read_exact(pid, address + 0x3C, &mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn module_base_and_identity(reader: &MemoryReader, pid: u32) -> Result<u64, String> {
    let process = OwnedHandle(
        unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                false,
                pid,
            )
        }
        .map_err(|error| format!("Open APB process failed: {error}"))?,
    );
    if unsafe { WaitForSingleObject(process.0, 0) } != windows::Win32::Foundation::WAIT_TIMEOUT {
        return Err("APB process is no longer running.".to_string());
    }

    let mut name = vec![0u16; 32768];
    let mut length = name.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_WIN32,
            PWSTR(name.as_mut_ptr()),
            &mut length,
        )
    }
    .map_err(|error| format!("Read APB executable path failed: {error}"))?;
    let path = PathBuf::from(String::from_utf16_lossy(&name[..length as usize]));
    if !path
        .file_name()
        .is_some_and(|file| file.to_string_lossy().eq_ignore_ascii_case("APB.exe"))
    {
        return Err(format!(
            "PID {pid} belongs to {}, not APB.exe.",
            path.display()
        ));
    }
    let mut file = File::open(&path)
        .map_err(|error| format!("Open APB executable {} failed: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut block = [0u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut block)
            .map_err(|error| format!("Hash APB executable failed: {error}"))?;
        if count == 0 {
            break;
        }
        hasher.update(&block[..count]);
    }
    let digest = format!("{:X}", hasher.finalize());
    if digest != EXPECTED_SHA256 {
        return Err(format!(
            "Unverified APB.exe build SHA-256 {digest}; expected {EXPECTED_SHA256}. Memory source cannot start."
        ));
    }

    let mut basic = ProcessBasicInformation::default();
    let mut returned = 0u32;
    let status = unsafe {
        NtQueryInformationProcess(
            process.0,
            0,
            &mut basic,
            std::mem::size_of::<ProcessBasicInformation>() as u32,
            &mut returned,
        )
    };
    if status != 0 || basic.peb.is_null() {
        return Err(format!(
            "Locate APB PEB failed: NTSTATUS 0x{:08X}.",
            status as u32
        ));
    }
    let base = read_u64(reader, pid, basic.peb as u64 + 0x10)?;
    let mut mz = [0u8; 2];
    reader.read_exact(pid, base, &mut mz)?;
    if mz != *b"MZ" {
        return Err(format!("APB module base 0x{base:X} has no MZ header."));
    }
    Ok(base)
}

fn camera_snapshot(
    reader: &MemoryReader,
    pid: u32,
    camera: u64,
) -> Result<CameraProjection, String> {
    if read_class(reader, pid, camera)? != CAMERA_CLASS {
        return Err("APB camera object class changed.".to_string());
    }
    let mut bytes = [0u8; 0x50];
    reader.read_exact(pid, camera + 0x680, &mut bytes)?;
    let matrix = std::array::from_fn(|index| {
        f32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap())
    });
    let position = parse_position(&bytes, 0x40)?;
    if !matrix.iter().all(|value: &f32| value.is_finite()) {
        return Err("APB camera matrix contains nonfinite values.".to_string());
    }
    Ok(CameraProjection { position, matrix })
}

fn position_object(reader: &MemoryReader, pid: u32, address: u64) -> Result<[f32; 3], String> {
    let mut bytes = [0u8; 0x58];
    reader.read_exact(pid, address + 0x3C, &mut bytes)?;
    if parse_u32(&bytes, 0) != POSITION_CLASS {
        return Err(format!(
            "APB position object at 0x{address:X} changed class."
        ));
    }
    parse_position(&bytes, 0x88 - 0x3C)
}

pub fn discover(reader: &MemoryReader, pid: u32, base: u64) -> Result<Discovery, String> {
    let table_address = base + TABLE_RVA;
    let mut table = vec![0u8; TABLE_ENTRIES * 8];
    for (chunk_index, chunk) in table.chunks_mut(4096).enumerate() {
        reader.read_exact(pid, table_address + (chunk_index * 4096) as u64, chunk)?;
    }
    let mut cameras = Vec::new();
    let mut controllers = Vec::new();
    let mut owners = Vec::new();
    let mut readable_headers = 0;
    let mut failed_headers = 0;
    for index in 0..TABLE_ENTRIES {
        let address = parse_u64(&table, index * 8);
        if !(0x10000..0x800000000000).contains(&address) || address % 8 != 0 {
            continue;
        }
        let mut header = [0u8; 0x40];
        if reader.read_exact(pid, address, &mut header).is_err() {
            failed_headers += 1;
            continue;
        }
        readable_headers += 1;
        match parse_u32(&header, 0x3C) {
            CAMERA_CLASS => cameras.push((index, address)),
            CONTROLLER_CLASS => controllers.push((index, address)),
            OWNER_CLASS => owners.push((index, address)),
            _ => {}
        }
    }
    let [(_, camera)] = cameras.as_slice() else {
        return Err(format!(
            "Expected one APB camera object; found {}.",
            cameras.len()
        ));
    };
    let camera = *camera;
    let camera_data = camera_snapshot(reader, pid, camera)?;
    let mut chains = Vec::new();
    let mut controller_debug = Vec::new();
    for (_, controller) in controllers {
        let mut bytes = [0u8; 0x288];
        reader.read_exact(pid, controller + 0x140, &mut bytes)?;
        let first = parse_u64(&bytes, 0);
        let local_link = parse_u64(&bytes, 8);
        let second = parse_u64(&bytes, 0x280);
        controller_debug.push(format!(
            "0x{controller:X}: first=0x{first:X} second=0x{second:X} link=0x{local_link:X}"
        ));
        if first != second || !(0x10000..0x800000000000).contains(&first) || first % 8 != 0 {
            continue;
        }
        let position = match position_object(reader, pid, first) {
            Ok(position) => position,
            Err(error) => {
                controller_debug.push(format!(
                    "0x{controller:X}: position 0x{first:X} rejected: {error}"
                ));
                continue;
            }
        };
        let link_class = if (0x10000..0x800000000000).contains(&local_link) && local_link % 8 == 0 {
            read_class(reader, pid, local_link).ok()
        } else {
            None
        };
        chains.push((controller, first, position, local_link, link_class));
    }
    let local_matches: Vec<_> = chains
        .iter()
        .filter(|(_, _, position, _, class)| {
            *class == Some(LOCAL_LINK_CLASS)
                && camera_data
                    .position
                    .iter()
                    .zip(position.iter())
                    .map(|(a, b)| (f64::from(*a) - f64::from(*b)).powi(2))
                    .sum::<f64>()
                    .sqrt()
                    <= 400.0
        })
        .collect();
    let [local] = local_matches.as_slice() else {
        let details = chains.iter().map(|(controller, position, xyz, link, class)| {
            let distance = camera_data.position.iter().zip(xyz.iter())
                .map(|(a, b)| (f64::from(*a) - f64::from(*b)).powi(2))
                .sum::<f64>().sqrt();
            format!("controller=0x{controller:X} position=0x{position:X} xyz={xyz:?} link=0x{link:X} class={class:?} distance={distance:.1}")
        }).collect::<Vec<_>>().join("; ");
        return Err(format!(
            "Cannot uniquely identify local APB controller: {} class-{LOCAL_LINK_CLASS} candidates near camera 0x{camera:X} at {:?}. Headers {readable_headers} read / {failed_headers} failed. Controllers: {}. Chains: {details}",
            local_matches.len(), camera_data.position, controller_debug.join("; "),
        ));
    };
    let local_controller = local.0;
    let local_position = local.1;
    let local_link = local.3;

    let mut actors: HashSet<u64> = chains
        .iter()
        .map(|(_, position, _, _, _)| *position)
        .filter(|position| *position != local_position)
        .collect();
    let mut skipped_owners = 0;
    for (_, owner) in owners {
        let mut bytes = [0u8; 0x148];
        if reader.read_exact(pid, owner + 0xA0, &mut bytes).is_err() {
            skipped_owners += 1;
            continue;
        }
        for offset in [0, 0x140] {
            let position = parse_u64(&bytes, offset);
            if !(0x10000..0x800000000000).contains(&position)
                || position % 8 != 0
                || position == local_position
            {
                continue;
            }
            if position_object(reader, pid, position).is_ok() {
                actors.insert(position);
            }
        }
    }
    if read_u64(reader, pid, table_address + cameras[0].0 as u64 * 8)? != camera {
        return Err("APB camera table slot changed during discovery.".to_string());
    }
    let mut actors: Vec<u64> = actors.into_iter().collect();
    actors.sort_unstable();
    Ok(Discovery {
        pid,
        base,
        camera,
        local_controller,
        local_position,
        local_link,
        actors,
        readable_headers,
        failed_headers,
        skipped_owners,
    })
}

impl ApbMemorySource {
    pub fn connect(pid: u32) -> Result<Self, String> {
        let reader = MemoryReader::connect()?;
        let base = module_base_and_identity(&reader, pid)?;
        let discovery = discover(&reader, pid, base)?;
        let window = unsafe { FindWindowW(None, w!("APB Reloaded")) };
        if window.0 == 0 {
            return Err("APB Reloaded window was not found.".to_string());
        }
        let mut window_pid = 0u32;
        unsafe { GetWindowThreadProcessId(window, Some(&mut window_pid)) };
        if window_pid != pid {
            return Err(format!(
                "APB window belongs to PID {window_pid}, expected {pid}."
            ));
        }
        Ok(Self {
            reader,
            discovery,
            window,
            warned_stale: HashSet::new(),
        })
    }

    pub fn discovery(&self) -> &Discovery {
        &self.discovery
    }

    pub fn replace_discovery(&mut self, discovery: Discovery) -> Result<(), String> {
        if discovery.pid != self.discovery.pid || discovery.base != self.discovery.base {
            return Err("APB discovery belongs to another process or module base.".to_string());
        }
        self.discovery = discovery;
        self.warned_stale.clear();
        Ok(())
    }

    /// Returns a centered APB client point in the same coordinate convention
    /// as the color detector. A missing player in view returns `None`.
    pub fn sample(
        &mut self,
        grab_width: i32,
        grab_height: i32,
        grab_shift_x: i32,
        grab_shift_y: i32,
    ) -> Result<Option<Target>, String> {
        if grab_width <= 0 || grab_height <= 0 {
            return Err("Memory source grab dimensions must be positive.".to_string());
        }
        if unsafe { GetForegroundWindow() } != self.window {
            return Ok(None);
        }
        let mut rect = RECT::default();
        unsafe { GetClientRect(self.window, &mut rect) }
            .map_err(|error| format!("Get APB client rectangle failed: {error}"))?;
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width < 320 || height < 240 {
            return Err(format!("APB client size is invalid: {width}x{height}."));
        }
        if read_class(
            &self.reader,
            self.discovery.pid,
            self.discovery.local_controller,
        )? != CONTROLLER_CLASS
            || read_u64(
                &self.reader,
                self.discovery.pid,
                self.discovery.local_controller + 0x140,
            )? != self.discovery.local_position
            || read_u64(
                &self.reader,
                self.discovery.pid,
                self.discovery.local_controller + 0x148,
            )? != self.discovery.local_link
            || read_class(&self.reader, self.discovery.pid, self.discovery.local_link)?
                != LOCAL_LINK_CLASS
        {
            return Err(
                "APB local-controller relationship changed; stop and restart Memory mode."
                    .to_string(),
            );
        }
        position_object(
            &self.reader,
            self.discovery.pid,
            self.discovery.local_position,
        )?;
        let camera = camera_snapshot(&self.reader, self.discovery.pid, self.discovery.camera)?;
        let mut best: Option<(f64, f64, Target)> = None;
        for &address in &self.discovery.actors {
            let position = match position_object(&self.reader, self.discovery.pid, address) {
                Ok(position) => position,
                Err(error) => {
                    // Actors can leave the district between discovery scans.
                    if self.warned_stale.insert(address) {
                        crate::log_err!(
                            "Memory actor 0x{address:X} became unreadable ({error}); awaiting next discovery refresh."
                        );
                    }
                    continue;
                }
            };
            let Some(projected) = camera.project(position, width as u32, height as u32)? else {
                continue;
            };
            let x = projected.x - f64::from(width) / 2.0 - f64::from(grab_shift_x);
            let y = projected.y - f64::from(height) / 2.0 - f64::from(grab_shift_y);
            if x.abs() > f64::from(grab_width) / 2.0 || y.abs() > f64::from(grab_height) / 2.0 {
                continue;
            }
            let target = Target {
                x: x.round() as i32,
                y: y.round() as i32,
                width: 1,
                height: 1,
                aspect: 0.0,
                seen: true,
            };
            let score = (x.abs(), y.abs());
            if best
                .as_ref()
                .is_none_or(|(best_x, best_y, _)| score < (*best_x, *best_y))
            {
                best = Some((score.0, score.1, target));
            }
        }
        Ok(best.map(|(_, _, target)| target))
    }
}

pub fn refresh(reader: &MemoryReader, current: &Discovery) -> Result<Discovery, String> {
    discover(reader, current.pid, current.base)
}
