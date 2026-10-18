//! Verified APB camera reacquisition and canonical pawn projections through EchoDriver.
use crate::lattice_encoding::{EncodedAddress, EncodedOffset};
use crate::apb_orchid::{
    self, field, kind, pointer, read, signature, typed, MissionRole, PawnRef, CAMERA,
};
use crate::apb_grove::{read_roster, RosterChangeMonitor, RosterLink, RosterRead};
use crate::apb_prism::{self, RenderRecency};
use crate::config::{LanternFilter, OrbitOrder};
use crate::detection::Target;
use crate::atlas_unicorn::AtlasUnicornTarget;
use crate::atlas_lantern::CameraProjection;
use crate::atlas_reader::AtlasReader;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::ffi::c_void;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use windows::core::{w, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetClientRect, GetForegroundWindow, GetWindowThreadProcessId,
};
pub const EXPECTED_SHA256: &str =
    "35AEF01734AAB20B4B0C622ADDF5706F9E75CAAA98D5B84A0EA14D9DF72EE453";
const CAMERA_TABLE_RVA: EncodedOffset = EncodedOffset::new(0x3DCE240);
pub const ROSTER_REFRESH_INTERVAL: Duration = Duration::from_millis(500);
pub const ROSTER_RETRY_INTERVAL: Duration = Duration::from_millis(50);
pub const ROSTER_CHANGE_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MAX_ROSTER_AGE: Duration = Duration::from_secs(2);
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
fn read_u64(reader: &AtlasReader, pid: u32, address: u64) -> Result<u64, String> {
    field(reader, pid, address)
}
pub(crate) fn module_base_and_identity(reader: &AtlasReader, pid: u32) -> Result<u64, String> {
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
    if cfg!(feature = "apb-hash-test") {
        crate::logger::log_message(
            "WARN",
            &format!("TEST BUILD: APB.exe hash gate disabled by user request; actual SHA-256 {digest}; expected {EXPECTED_SHA256}."),
        );
    } else if digest != EXPECTED_SHA256 {
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
    let base = read_u64(reader, pid, basic.peb as u64 + const { EncodedOffset::new(0x10) }.get())?;
    let mut mz = [0u8; 2];
    reader.read_exact(pid, base, &mut mz)?;
    if mz != *b"MZ" {
        return Err(format!("APB module base 0x{base:X} has no MZ header."));
    }
    Ok(base)
}

#[derive(Debug, Clone)]
pub struct Discovery {
    pub pid: u32,
    pub base: EncodedAddress,
    pub actors: Vec<PawnRef>,
    pub nodes: usize,
    pub vehicles: usize,
    pub head: Option<EncodedAddress>,
    pub world_instance: Option<PawnRef>,
    pub captured_at: Instant,
    pub waiting_reason: Option<String>,
    links: Vec<RosterLink>,
    camera: Option<CameraRef>,
    camera_audited_at: Instant,
}

impl Discovery {
    fn waiting(pid: u32, base: u64, reason: String) -> Self {
        Self {
            pid,
            base: EncodedAddress::new(base).expect("MemoryReader initializes encoding before discovery"),
            actors: Vec::new(),
            nodes: 0,
            vehicles: 0,
            head: None,
            world_instance: None,
            captured_at: Instant::now(),
            waiting_reason: Some(reason),
            links: Vec::new(),
            camera: None,
            camera_audited_at: Instant::now(),
        }
    }
}

fn roster_world(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    camera: CameraRef,
) -> Result<Option<PawnRef>, String> {
    if !camera_binding_matches(reader, pid, base, camera)?
        || typed(reader, pid, base, camera.address.get(), CAMERA)? != camera.signature
    {
        return Err("Camera binding/instance changed during roster read.".into());
    }
    let world = field(reader, pid, camera.address.get() + const { EncodedOffset::new(0x2E0) }.get())?;
    if world == 0 {
        return Ok(None);
    }
    apb_orchid::world_instance(reader, pid, base, world).map(Some)
}

// Follow the stored World head and pawn next pointers. Never substitute a module
// reference survey when this selected traversal is unavailable or unsupported.
pub fn discover(reader: &AtlasReader, pid: u32, base: u64) -> Result<Discovery, String> {
    discover_from(reader, pid, base, None)
}

fn discover_from(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    previous: Option<&Discovery>,
) -> Result<Discovery, String> {
    let captured_at = Instant::now();
    // Full uniqueness audits run in the roster worker, never on the frame timer.
    let audit =
        previous.is_none_or(|old| old.camera_audited_at.elapsed() >= Duration::from_secs(2));
    let camera = if audit {
        locate_camera(reader, pid, base)?
    } else {
        resolve_camera(reader, pid, base, previous.and_then(|old| old.camera))?
    };
    let Some(camera) = camera else {
        return Ok(Discovery::waiting(
            pid,
            base,
            "District camera/World unavailable".into(),
        ));
    };
    let Some(world) = roster_world(reader, pid, base, camera)? else {
        return Ok(Discovery::waiting(
            pid,
            base,
            "District World unavailable".into(),
        ));
    };
    let roster = match read_roster(base, world, |address, bytes| {
        reader.read_exact(pid, address, bytes)
    })? {
        RosterRead::Complete(roster) => roster,
        RosterRead::Changed(reason) => return Ok(Discovery::waiting(pid, base, reason)),
    };
    let Some(final_camera) = resolve_camera(reader, pid, base, Some(camera))? else {
        return Ok(Discovery::waiting(
            pid,
            base,
            "Camera disappeared during rooted roster read".into(),
        ));
    };
    if !camera.same_instance(final_camera)
        || roster_world(reader, pid, base, final_camera)? != Some(world)
    {
        return Ok(Discovery::waiting(
            pid,
            base,
            "Camera/World changed during rooted roster read".into(),
        ));
    }
    Ok(Discovery {
        pid,
        base: EncodedAddress::new(base)?,
        actors: roster.actors,
        nodes: roster.nodes,
        vehicles: roster.vehicles,
        head: Some(roster.head),
        world_instance: Some(world),
        captured_at,
        waiting_reason: None,
        links: roster.links,
        camera: Some(final_camera),
        camera_audited_at: if audit {
            captured_at
        } else {
            previous.unwrap().camera_audited_at
        },
    })
}
pub fn refresh(reader: &AtlasReader, current: &Discovery) -> Result<Discovery, String> {
    discover_from(reader, current.pid, current.base.get(), Some(current))
}

pub fn roster_changed(
    reader: &AtlasReader,
    current: &Discovery,
    monitor: &mut RosterChangeMonitor,
) -> Result<bool, String> {
    let (Some(world), Some(head)) = (current.world_instance, current.head) else {
        return Ok(false);
    };
    monitor.changed(
        current.base.get(),
        world,
        head.get(),
        &current.links,
        |address, bytes| reader.read_exact(current.pid, address, bytes),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CameraRef {
    table: EncodedAddress,
    index: usize,
    address: EncodedAddress,
    signature: [u8; 36],
}
impl CameraRef {
    fn same_instance(self, other: Self) -> bool {
        self.address.get() == other.address.get() && self.signature == other.signature
    }
}

fn camera_binding_matches(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    camera: CameraRef,
) -> Result<bool, String> {
    Ok(field(reader, pid, base + CAMERA_TABLE_RVA.get())? == camera.table.get()
        && field(reader, pid, camera.table.get() + camera.index as u64 * 8)? == camera.address.get())
}

fn resolve_camera(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    known: Option<CameraRef>,
) -> Result<Option<CameraRef>, String> {
    if let Some(known) = known {
        if camera_binding_matches(reader, pid, base, known)? {
            return Ok(Some(known));
        }
        // A slot is a table binding, not the object's identity. Locate this exact
        // instance in one table read and verify it before keeping tracking live.
        let table = field(reader, pid, base + CAMERA_TABLE_RVA.get())?;
        if pointer(table) {
            let entries = read::<1024>(reader, pid, table)?;
            let indices: Vec<_> = entries
                .chunks_exact(8)
                .enumerate()
                .filter_map(|(index, entry)| {
                    (apb_orchid::u64_at(entry, 0) == known.address.get()).then_some(index)
                })
                .collect();
            if let [index] = indices.as_slice() {
                let rebound = CameraRef {
                    table: EncodedAddress::new(table)?,
                    index: *index,
                    ..known
                };
                if typed(reader, pid, base, known.address.get(), CAMERA)? == known.signature
                    && camera_binding_matches(reader, pid, base, rebound)?
                {
                    return Ok(Some(rebound));
                }
            }
        }
    }
    locate_camera(reader, pid, base)
}
fn camera_snapshot(
    reader: &AtlasReader,
    pid: u32,
    base: u64,
    camera: CameraRef,
) -> Result<CameraProjection, String> {
    if typed(reader, pid, base, camera.address.get(), CAMERA)? != camera.signature {
        return Err("Camera instance changed.".into());
    }
    let bytes = read::<0x50>(reader, pid, camera.address.get() + const { EncodedOffset::new(0x680) }.get())?;
    let matrix =
        std::array::from_fn(|i| f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap()));
    let position = std::array::from_fn(|i| {
        f32::from_le_bytes(bytes[0x40 + i * 4..0x44 + i * 4].try_into().unwrap())
    });
    if !matrix.iter().chain(position.iter()).all(|v| v.is_finite())
        || (matrix[14] + 9.99).abs() > 0.02
        || matrix[15].abs() > 0.02
        || position.iter().any(|v| v.abs() >= 1_000_000.0)
    {
        return Err("APB camera matrix/position failed validated layout checks.".into());
    }
    if typed(reader, pid, base, camera.address.get(), CAMERA)? != camera.signature {
        return Err("Camera changed during snapshot.".into());
    }
    Ok(CameraProjection { position, matrix })
}
fn locate_camera(reader: &AtlasReader, pid: u32, base: u64) -> Result<Option<CameraRef>, String> {
    let table = field(reader, pid, base + CAMERA_TABLE_RVA.get())?;
    if table == 0 {
        return Ok(None);
    }
    if !pointer(table) {
        return Err(format!("APB camera table pointer is invalid: 0x{table:X}"));
    }
    let entries = read::<1024>(reader, pid, table)?;
    let mut found = Vec::new();
    for (index, entry) in entries.chunks_exact(8).enumerate() {
        let address = apb_orchid::u64_at(entry, 0);
        if !pointer(address) {
            continue;
        }
        let Ok(header) = read::<64>(reader, pid, address) else {
            continue;
        };
        if kind(&header, base) == CAMERA {
            let camera = CameraRef {
                table: EncodedAddress::new(table)?,
                index,
                address: EncodedAddress::new(address)?,
                signature: signature(&header),
            };
            camera_snapshot(reader, pid, base, camera)?;
            found.push(camera);
        }
    }
    // Audit current camera candidates, not equality of an entire dynamic table.
    // Non-camera objects can be inserted/removed while the camera stays valid.
    if field(reader, pid, base + CAMERA_TABLE_RVA.get())? != table {
        return Err("Camera table root moved during lookup; waiting for a stable binding.".into());
    }
    let current_entries = read::<1024>(reader, pid, table)?;
    let mut current = Vec::new();
    for (index, entry) in current_entries.chunks_exact(8).enumerate() {
        let address = apb_orchid::u64_at(entry, 0);
        if !pointer(address) { continue; }
        let Ok(header) = read::<64>(reader, pid, address) else { continue; };
        if kind(&header, base) == CAMERA {
            let camera = CameraRef { table: EncodedAddress::new(table)?, index, address: EncodedAddress::new(address)?, signature: signature(&header) };
            camera_snapshot(reader, pid, base, camera)?;
            current.push(camera);
        }
    }
    if current.len() != found.len()
        || current.iter().any(|camera| !found.iter().any(|old| old.same_instance(*camera)))
    {
        return Err("Typed camera candidates changed during lookup; waiting for a stable camera.".into());
    }
    if let [camera] = current.as_slice() {
        if !camera_binding_matches(reader, pid, base, *camera)?
            || typed(reader, pid, base, camera.address.get(), CAMERA)? != camera.signature
        {
            return Err("Selected camera binding/instance changed during lookup.".into());
        }
        if entries != current_entries {
            crate::log_info!("Camera table entries changed; unique verified camera retained at 0x{:X}, table 0x{:X}, slot {}.", camera.address.get(), table, camera.index);
        }
    } else if field(reader, pid, base + CAMERA_TABLE_RVA.get())? != table {
        return Err("Camera table root changed during empty lookup.".into());
    }
    match current.as_slice() {
        [] => Ok(None),
        [camera] => Ok(Some(*camera)),
        _ => Err(format!("Expected one typed camera; found {}.", current.len())),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectedPawn {
    pub address: EncodedAddress,
    pub x: f64,
    pub y: f64,
    pub role: MissionRole,
    pub pawn: PawnRef,
    pub body_bounds: Option<[f64; 4]>,
    pub distance_units: Option<f64>,
    pub render_recency: Option<RenderRecency>,
    pub stable_recency: bool,
}
#[derive(Clone, Copy)]
pub struct AtlasFrameRequest {
    pub unicorn: Option<LanternFilter>,
    pub visible_only: bool,
    pub comet: Option<LanternFilter>,
    pub projections: Option<LanternFilter>,
    pub esp_boxes: bool,
    pub distances: bool,
}

impl AtlasFrameRequest {
    fn unicorn_includes(self, role: MissionRole) -> bool {
        role != MissionRole::Unknown && self.unicorn.is_some_and(|filter| filter.includes(role))
    }
}
pub struct CometPawn {
    pub position: [f32; 3],
    pub role: MissionRole,
}

pub struct AtlasFrame {
    pub world: PawnRef,
    pub visible_only: bool,
    pub comet_pawns: Vec<CometPawn>,
    pub comet_origin: Option<[f32; 3]>,
    pub camera: CameraProjection,
    pub points: Vec<ProjectedPawn>,
    pub client_rect: RECT,
}

#[derive(Default)]
struct FrameCounts {
    opponents: usize,
    teammates: usize,
    outside: usize,
    unknown: usize,
    filtered: usize,
    ownership_rejected: usize,
    wrong_world: usize,
    projection_rejected: usize,
    bounds_rejected: usize,
    projected: usize,
    eligible: usize,
    stale_render: usize,
}
impl AtlasFrame {
    pub fn target(
        &self,
        grab_width: i32,
        grab_height: i32,
        shift_x: i32,
        shift_y: i32,
    ) -> Option<AtlasUnicornTarget> {
        self.target_with_priority(
            grab_width,
            grab_height,
            shift_x,
            shift_y,
            OrbitOrder::ClosestUnicorn,
            LanternFilter::Opponents,
        )
    }

    pub fn target_with_priority(
        &self,
        grab_width: i32,
        grab_height: i32,
        shift_x: i32,
        shift_y: i32,
        priority: OrbitOrder,
        filter: LanternFilter,
    ) -> Option<AtlasUnicornTarget> {
        let width = self.client_rect.right - self.client_rect.left;
        let height = self.client_rect.bottom - self.client_rect.top;
        self.points
            .iter()
            .filter(|point| !self.visible_only || point.stable_recency)
            .filter(|point| point.role != MissionRole::Unknown && filter.includes(point.role))
            .filter_map(|point| {
                let x = point.x - f64::from(width) / 2.0 - f64::from(shift_x);
                let y = point.y - f64::from(height) / 2.0 - f64::from(shift_y);
                let bounds = point.body_bounds?;
                if priority == OrbitOrder::ClosestDistance && point.distance_units.is_none() {
                    return None;
                }
                let left = bounds[0] - f64::from(width) / 2.0 - f64::from(shift_x);
                let right = bounds[2] - f64::from(width) / 2.0 - f64::from(shift_x);
                let top = bounds[1] - f64::from(height) / 2.0;
                let bottom = bounds[3] - f64::from(height) / 2.0;
                if left > f64::from(grab_width) / 2.0
                    || right < -f64::from(grab_width) / 2.0
                    || top - f64::from(shift_y) > f64::from(grab_height) / 2.0
                    || bottom - f64::from(shift_y) < -f64::from(grab_height) / 2.0
                {
                    return None;
                }
                let dx = (bounds[0] + bounds[2] - f64::from(width)) / 2.0;
                let dy = (bounds[1] + bounds[3] - f64::from(height)) / 2.0;
                let unicorn_distance = dx * dx + dy * dy;
                let rank = match priority {
                    OrbitOrder::ClosestUnicorn => unicorn_distance,
                    OrbitOrder::ClosestDistance => point.distance_units.unwrap(),
                };
                Some((
                    rank,
                    unicorn_distance,
                    AtlasUnicornTarget {
                        point: Target {
                            x: x.round() as i32,
                            y: y.round() as i32,
                            width: (bounds[2] - bounds[0]).ceil().max(1.0) as i32,
                            height: (bounds[3] - bounds[1]).ceil().max(1.0) as i32,
                            aspect: 0.0,
                            seen: true,
                        },
                        pawn: point.pawn,
                        top,
                        bottom,
                        body_bounds: bounds,
                        distance_units: point.distance_units,
                    },
                ))
            })
            .min_by(|a, b| {
                a.0.total_cmp(&b.0)
                    .then(a.1.total_cmp(&b.1))
                    .then(a.2.pawn.address.get().cmp(&b.2.pawn.address.get()))
            })
            .map(|(_, _, target)| target)
    }
}
pub struct AtlasSource {
    reader: AtlasReader,
    discovery: Discovery,
    window: HWND,
    process: OwnedHandle,
    camera: Option<CameraRef>,
    pending: Option<CameraRef>,
    next_lookup: Instant,
    waiting: Option<(String, Instant)>,
    unexpected_since: Option<Instant>,
    rejected: HashSet<PawnRef>,
    unknown: HashSet<PawnRef>,
    invalid_bounds: HashSet<PawnRef>,
    refresh_requested: bool,
    refresh_requested_for: Option<PawnRef>,
    next_diagnostics: Instant,
    distance_issue: Option<String>,
    stable_recency: apb_prism::StableRecency,
}
impl AtlasSource {
    pub fn connect(pid: u32) -> Result<Self, String> {
        let reader = AtlasReader::connect()?;
        let base = module_base_and_identity(&reader, pid)?;
        let discovery = discover(&reader, pid, base)?;
        let window = unsafe { FindWindowW(None, w!("APB Reloaded")) };
        if window.0 == 0 {
            return Err("APB window not found. Open APB and restart Verux processing.".into());
        }
        let mut window_pid = 0;
        unsafe {
            GetWindowThreadProcessId(window, Some(&mut window_pid));
        }
        if window_pid != pid {
            return Err("APB window/process identity differs. Refresh APB and retry.".into());
        }
        let process = OwnedHandle(
            unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }
                .map_err(|e| format!("Monitor APB process failed: {e}"))?,
        );
        let now = Instant::now();
        let camera = discovery.camera;
        Ok(Self {
            reader,
            discovery,
            window,
            process,
            camera,
            pending: None,
            next_lookup: now,
            waiting: None,
            unexpected_since: None,
            rejected: HashSet::new(),
            unknown: HashSet::new(),
            invalid_bounds: HashSet::new(),
            refresh_requested: false,
            refresh_requested_for: None,
            next_diagnostics: now,
            distance_issue: None,
            stable_recency: apb_prism::StableRecency::default(),
        })
    }
    pub fn discovery(&self) -> &Discovery {
        &self.discovery
    }
    pub fn object_points(
        &self,
        workers: &crate::apb_glow::ObjectWorkers,
        world: PawnRef,
        camera_position: [f32; 3],
        radius: f64,
    ) -> Result<Vec<crate::apb_glow::ObjectPoint>, String> {
        workers.points(&self.reader, world, camera_position, radius)
    }
    pub fn take_refresh_request(&mut self) -> bool {
        std::mem::take(&mut self.refresh_requested)
    }
    fn request_refresh(&mut self, world: PawnRef) {
        if self.refresh_requested_for != Some(world) {
            self.refresh_requested = true;
            self.refresh_requested_for = Some(world);
        }
    }
    pub fn replace_discovery(&mut self, discovery: Discovery) -> Result<(), String> {
        if discovery.pid != self.discovery.pid || discovery.base.get() != self.discovery.base.get() {
            return Err("Pawn roster belongs to another process.".into());
        }
        self.discovery = discovery;
        self.refresh_requested_for = None;
        let current: HashSet<_> = self.discovery.actors.iter().copied().collect();
        self.rejected.retain(|pawn| current.contains(pawn));
        self.unknown.retain(|pawn| current.contains(pawn));
        self.invalid_bounds.retain(|pawn| current.contains(pawn));
        self.stable_recency.retain(&current);
        Ok(())
    }
    fn withhold(
        &mut self,
        reason: String,
        unexpected: bool,
    ) -> Result<Option<AtlasFrame>, String> {
        self.stable_recency.clear();
        let now = Instant::now();
        let since = self
            .waiting
            .as_ref()
            .map(|(_, since)| *since)
            .unwrap_or(now);
        if self
            .waiting
            .as_ref()
            .is_none_or(|(previous, _)| previous != &reason)
        {
            if unexpected {
                crate::log_err!("Memory acquisition withheld: {reason}");
            } else {
                crate::log_info!("Memory acquisition withheld: {reason}");
            }
        }
        self.waiting = Some((reason.clone(), since));
        if unexpected {
            let since = *self.unexpected_since.get_or_insert(now);
            if now.duration_since(since) > Duration::from_secs(30) {
                return Err(format!("Memory reacquisition failed: {reason}. Return to a district and restart processing; inspect verux_apb.log."));
            }
        } else {
            self.unexpected_since = None;
        }
        Ok(None)
    }
    pub fn frame(&mut self) -> Result<Option<AtlasFrame>, String> {
        self.frame_for(AtlasFrameRequest {
            unicorn: Some(LanternFilter::Opponents),
            visible_only: true,
            comet: None,
            projections: Some(LanternFilter::AllPlayers),
            esp_boxes: true,
            distances: true,
        })
    }

    /// Classification precedes expensive reads. Input blocking never changes
    /// this request; it gates actions after acquisition.
    pub fn frame_for(
        &mut self,
        request: AtlasFrameRequest,
    ) -> Result<Option<AtlasFrame>, String> {
        if unsafe { WaitForSingleObject(self.process.0, 0) }
            != windows::Win32::Foundation::WAIT_TIMEOUT
        {
            return Err("APB exited. Restart APB and Verux processing.".into());
        }
        if unsafe { GetForegroundWindow() } != self.window {
            self.pending = None;
            self.next_lookup = Instant::now();
            return self.withhold("APB is not foreground".into(), false);
        }
        let pid = self.discovery.pid;
        let base = self.discovery.base.get();
        let now = Instant::now();
        if self.camera.is_none() && now < self.next_lookup {
            return Ok(None);
        }
        match resolve_camera(&self.reader, pid, base, self.camera) {
            Ok(Some(found)) => {
                if self.camera.is_some_and(|old| old.same_instance(found)) {
                    if self.camera != Some(found) {
                        crate::log_info!("Memory camera binding moved: 0x{:X}, table 0x{:X}, slot {}; same verified instance retained.", found.address.get(), found.table.get(), found.index);
                    }
                    self.camera = Some(found);
                } else if self.pending.is_some_and(|old| old.same_instance(found)) {
                    self.camera = Some(found);
                    self.pending = None;
                    crate::log_info!(
                        "Memory camera acquired: 0x{:X}, table 0x{:X}, slot {}",
                        found.address.get(),
                        found.table.get(),
                        found.index
                    );
                } else {
                    self.camera = None;
                    self.pending = Some(found);
                    self.next_lookup = now + Duration::from_millis(500);
                    return self.withhold("Confirming stable camera".into(), false);
                }
            }
            Ok(None) => {
                self.camera = None;
                self.pending = None;
                self.next_lookup = now + Duration::from_millis(500);
                return self.withhold("District camera unavailable".into(), false);
            }
            Err(error) => {
                self.camera = None;
                self.pending = None;
                self.next_lookup = now + Duration::from_millis(500);
                return self.withhold(error, true);
            }
        }
        let Some(camera_ref) = self.camera else {
            return Ok(None);
        };
        let camera = match camera_snapshot(&self.reader, pid, base, camera_ref) {
            Ok(v) => v,
            Err(error) => {
                self.camera = None;
                self.pending = None;
                return self.withhold(error, true);
            }
        };
        let local = match apb_orchid::local(&self.reader, pid, base, camera_ref.address.get()) {
            Ok(Some(local)) => local,
            Ok(None) => return self.withhold("District local ownership unavailable".into(), false),
            Err(error) => return self.withhold(error, true),
        };
        if self.discovery.world_instance != Some(local.world_instance()) {
            self.request_refresh(local.world_instance());
            return self.withhold(
                "Waiting for a rooted pawn roster of the current district world".into(),
                false,
            );
        }
        if self.discovery.captured_at.elapsed() > MAX_ROSTER_AGE {
            self.request_refresh(local.world_instance());
            return self.withhold(
                "Rooted pawn roster is older than two seconds; waiting for a stable refresh".into(),
                true,
            );
        }
        let local_mission = match local.pawn {
            Some(pawn) => match apb_orchid::mission(&self.reader, pid, base, pawn) {
                Ok(v) => Some(v),
                Err(error) => return self.withhold(error, true),
            },
            None => None,
        };
        let local_position = if request.distances || request.comet.is_some() {
            match apb_orchid::local_position(&self.reader, pid, base, local) {
                Ok(Some(position)) => {
                    if self.distance_issue.take().is_some() {
                        crate::log_info!(
                            "Memory distance measurement resumed from verified local pawn XYZ."
                        );
                    }
                    Some(position)
                }
                result => {
                    let reason = match &result {
                        Ok(None) => "Local character unavailable; distance labels and Closest Distance targeting withheld.".to_string(),
                        Err(error) => format!("Distance unavailable: {error}; distance labels and Closest Distance targeting withheld."),
                        _ => unreachable!(),
                    };
                    if self.distance_issue.as_ref() != Some(&reason) {
                        crate::log_err!("{reason}");
                        self.distance_issue = Some(reason.clone());
                    }
                    if request.comet.is_some() {
                        return self.withhold(
                            format!("Comet waiting for verified local character position: {reason} Return to your district character if this persists."),
                            matches!(result, Err(_)),
                        );
                    }
                    None
                }
            }
        } else {
            None
        };
        let mut rect = RECT::default();
        unsafe { GetClientRect(self.window, &mut rect) }
            .map_err(|e| format!("APB client rectangle failed: {e}"))?;
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width < 320 || height < 240 {
            return self.withhold("APB client viewport unavailable".into(), false);
        }
        let mut origin = POINT::default();
        if !unsafe { ClientToScreen(self.window, &mut origin) }.as_bool() {
            return Err("Locate APB client origin failed.".into());
        }
        let mut points = Vec::new();
        let mut comet_pawns = Vec::new();
        let mut counts = FrameCounts::default();
        for &pawn in &self.discovery.actors {
            if local
                .pawn
                .is_some_and(|local_pawn| pawn.address.get() == local_pawn.address.get())
            {
                continue;
            }
            let role = match apb_orchid::mission(&self.reader, pid, base, pawn) {
                Ok(remote) => {
                    let role = local_mission
                        .map(|fields| fields.classify(remote))
                        .unwrap_or(MissionRole::Unknown);
                    if role == MissionRole::Unknown && self.unknown.insert(pawn) {
                        crate::log_err!("Unclassified mission fields for pawn 0x{:X}: local {local_mission:?}, remote {remote:?}; excluded from unicorn.",pawn.address.get());
                    }
                    if role != MissionRole::Unknown {
                        self.unknown.remove(&pawn);
                    }
                    role
                }
                Err(error) => {
                    if self.unknown.insert(pawn) {
                        crate::log_err!(
                            "Pawn 0x{:X} mission sample withheld: {error}",
                            pawn.address.get()
                        );
                    }
                    MissionRole::Unknown
                }
            };
            match role {
                MissionRole::Opponent => counts.opponents += 1,
                MissionRole::Teammate => counts.teammates += 1,
                MissionRole::OutsideMission => counts.outside += 1,
                MissionRole::Unknown => counts.unknown += 1,
            }
            let unicorn_candidate = request.unicorn_includes(role);
            let displayed = request.projections.is_some_and(|filter| filter.includes(role));
            let comet_displayed = request.comet.is_some_and(|filter| filter.includes(role));
            if !unicorn_candidate && !displayed && !comet_displayed {
                counts.filtered += 1;
                continue;
            }
            let owned = match apb_orchid::owned_position(&self.reader, pid, base, pawn) {
                Ok(value) => value,
                Err(error) => {
                    counts.ownership_rejected += 1;
                    if self.rejected.insert(pawn) {
                        crate::log_err!("Pawn 0x{:X} withheld: {error}", pawn.address.get());
                    }
                    continue;
                }
            };
            if owned.world.get() != local.world.get() {
                counts.wrong_world += 1;
                if self.rejected.insert(pawn) {
                    crate::log_err!(
                        "Pawn 0x{:X} withheld: World 0x{:X} differs from local World 0x{:X}.",
                        pawn.address.get(),
                        owned.world.get(),
                        local.world.get()
                    );
                }
                continue;
            }
            if owned.controller.get() == local.controller.get() {
                continue;
            }
            if self.rejected.remove(&pawn) {
                crate::log_info!(
                    "Pawn 0x{:X} current ownership recovered; acquisition resumed.",
                    pawn.address.get()
                );
            }
            if comet_displayed {
                comet_pawns.push(CometPawn { position: owned.position, role });
            }
            if !unicorn_candidate && !displayed { continue; }
            let needs_bounds =
                unicorn_candidate || (displayed && request.esp_boxes);
            let projected = if needs_bounds {
                camera.project_unclipped(owned.position, width as u32, height as u32)?
            } else {
                camera.project(owned.position, width as u32, height as u32)?
            };
            if let Some(point) = projected {
                let body_bounds = if needs_bounds {
                    match apb_orchid::collision_bounds(&self.reader, pid, base, pawn) {
                        Ok(bounds) => {
                            self.invalid_bounds.remove(&pawn);
                            let mut screen = [
                                f64::INFINITY,
                                f64::INFINITY,
                                f64::NEG_INFINITY,
                                f64::NEG_INFINITY,
                            ];
                            let mut valid = true;
                            for mask in 0..8 {
                                let world = std::array::from_fn(|axis| {
                                    bounds.origin[axis]
                                        + bounds.extent[axis]
                                            * if mask & (1 << axis) == 0 { -1.0 } else { 1.0 }
                                });
                                match camera.project_unclipped(
                                    world,
                                    width as u32,
                                    height as u32,
                                )? {
                                    Some(p) => {
                                        screen[0] = screen[0].min(p.x);
                                        screen[1] = screen[1].min(p.y);
                                        screen[2] = screen[2].max(p.x);
                                        screen[3] = screen[3].max(p.y);
                                    }
                                    None => valid = false,
                                }
                            }
                            if valid && screen[3] > screen[1] && screen[2] > screen[0] {
                                Some(screen)
                            } else {
                                counts.bounds_rejected += 1;
                                None
                            }
                        }
                        Err(error) => {
                            counts.bounds_rejected += 1;
                            if self.invalid_bounds.insert(pawn) {
                                crate::log_err!("Pawn 0x{:X} has no verified current body bounds: {error}; memory unicorn/Aurora box withheld.",pawn.address.get());
                            }
                            None
                        }
                    }
                } else {
                    None
                };
                // Feet/actor origin can leave the viewport while the character's
                // verified body is still visible. Keep that body eligible.
                let origin_visible = point.x >= 0.0
                    && point.x <= f64::from(width)
                    && point.y >= 0.0
                    && point.y <= f64::from(height);
                let body_visible = body_bounds.is_some_and(|b| {
                    b[2] >= 0.0
                        && b[0] <= f64::from(width)
                        && b[3] >= 0.0
                        && b[1] <= f64::from(height)
                });
                if !origin_visible && !body_visible {
                    counts.projection_rejected += 1;
                    continue;
                }
                counts.projected += 1;
                if unicorn_candidate && body_bounds.is_some() {
                    counts.eligible += 1;
                }
                points.push(ProjectedPawn {
                    address: pawn.address,
                    x: point.x,
                    y: point.y,
                    role,
                    pawn,
                    body_bounds,
                    stable_recency: false,
                    render_recency: if unicorn_candidate && request.visible_only {
                        match apb_prism::sample(&self.reader, pid, base, pawn, local.world_instance()) {
                            Ok(value) => Some(value),
                            Err(error) => return self.withhold(format!("Visibility check unavailable for pawn 0x{:X}: {error}", pawn.address.get()), true),
                        }
                    } else { None },
                    distance_units: local_position.map(|local| {
                        local
                            .iter()
                            .zip(owned.position)
                            .map(|(a, b)| {
                                let delta = f64::from(*a) - f64::from(b);
                                delta * delta
                            })
                            .sum::<f64>()
                            .sqrt()
                    }),
                });
            } else {
                counts.projection_rejected += 1;
            }
        }
        match apb_orchid::local_unchanged(&self.reader, pid, base, camera_ref.address.get(), local) {
            Ok(true) => {}
            Ok(false) => return self.withhold("Local ownership changed during frame".into(), true),
            Err(error) => {
                return self.withhold(format!("Local ownership recheck failed: {error}"), true)
            }
        }
        if let Some(pawn) = local.pawn {
            match apb_orchid::mission(&self.reader, pid, base, pawn) {
                Ok(fields) if Some(fields) == local_mission => {}
                Ok(_) => return self.withhold("Local mission changed during frame".into(), true),
                Err(error) => {
                    return self.withhold(format!("Local mission recheck failed: {error}"), true)
                }
            }
        }
        let final_binding = resolve_camera(&self.reader, pid, base, Some(camera_ref))
            .and_then(|bound| match bound {
                Some(bound) if bound.same_instance(camera_ref) => {
                    typed(&self.reader, pid, base, bound.address.get(), CAMERA)
                        .map(|signature| (signature == bound.signature).then_some(bound))
                }
                _ => Ok(None),
            });
        match final_binding {
            Ok(Some(bound))
                if bound.same_instance(camera_ref)
                    =>
            {
                self.camera = Some(bound)
            }
            result => {
                self.camera = None;
                self.pending = None;
                self.next_lookup = Instant::now();
                let (reason, unexpected) = match result {
                    Err(error) => (format!("Camera recheck failed: {error}"), true),
                    _ => ("Camera instance changed during frame; waiting for stable camera".into(), false),
                };
                return self.withhold(reason, unexpected);
            }
        }
        if unsafe { GetForegroundWindow() } != self.window {
            self.stable_recency.clear();
            return Ok(None);
        }
        if self.discovery.captured_at.elapsed() > MAX_ROSTER_AGE {
            self.request_refresh(local.world_instance());
            return self.withhold(
                "Rooted pawn roster expired during frame; waiting for a stable refresh".into(),
                true,
            );
        }
        if request.unicorn.is_some() && request.visible_only {
            // Rebase all pawn ages at frame completion, rather than granting
            // eligibility from a clock sampled before a long roster pass.
            let clock = match apb_prism::world_clock(&self.reader, pid, base, local.world_instance()) {
                Ok(value) => value,
                Err(error) => return self.withhold(format!("Visibility frame clock unavailable: {error}"), true),
            };
            counts.eligible = 0;
            for point in &mut points {
                if let Some(value) = point.render_recency {
                    match value.at_world_time(clock) {
                        Ok(value) => {
                            point.render_recency = Some(value);
                            point.stable_recency = self.stable_recency.eligible(local.world_instance(), point.pawn, value);
                            if !value.recently_rendered { counts.stale_render += 1; }
                            if request.unicorn_includes(point.role) && point.body_bounds.is_some() && point.stable_recency {
                                counts.eligible += 1;
                            }
                        }
                        Err(error) => return self.withhold(error, true),
                    }
                }
            }
            self.stable_recency.retain(&points.iter().filter(|point| point.render_recency.is_some()).map(|point| point.pawn).collect());
        } else {
            self.stable_recency.clear();
        }
        self.unexpected_since = None;
        if self.waiting.take().is_some() {
            crate::log_info!("Memory acquisition resumed with verified camera/local ownership.");
        }
        if Instant::now() >= self.next_diagnostics {
            self.next_diagnostics = Instant::now() + Duration::from_secs(5);
            crate::log_info!("Memory acquisition: roster={} characters/{} vehicles, age={} ms; roles opponents={}, teammates={}, outside={}, unknown={}; filtered={}, ownership_rejected={}, wrong_world={}, projection_rejected={}, bounds_rejected={}; projected={}, eligible={}, stale_render={}, visible_only={}; frame={} ms.",
                self.discovery.actors.len(), self.discovery.vehicles, self.discovery.captured_at.elapsed().as_millis(),
                counts.opponents, counts.teammates, counts.outside, counts.unknown, counts.filtered,
                counts.ownership_rejected, counts.wrong_world, counts.projection_rejected, counts.bounds_rejected,
                counts.projected, counts.eligible, counts.stale_render, request.unicorn.is_some() && request.visible_only, now.elapsed().as_millis());
        }
        rect.left = origin.x;
        rect.top = origin.y;
        rect.right = origin.x + width;
        rect.bottom = origin.y + height;
        Ok(Some(AtlasFrame {
            world: local.world_instance(),
            visible_only: request.unicorn.is_some() && request.visible_only,
            comet_pawns,
            comet_origin: local_position,
            camera,
            points,
            client_rect: rect,
        }))
    }
}
