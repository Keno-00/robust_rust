//! Read-only APB cursor state from the verified APB.exe build through EchoDriver.

use crate::lattice_encoding::{EncodedAddress, EncodedOffset};
use std::time::{Duration, Instant};

use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_TIMEOUT};
use windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetForegroundWindow, GetWindowThreadProcessId,
};

use crate::apb_atlas::module_base_and_identity;
use crate::atlas_reader::AtlasReader;

const CURSOR_POINTER_RVA: EncodedOffset = EncodedOffset::new(0x3BD3340);
const CURSOR_FIELD_OFFSET: EncodedOffset = EncodedOffset::new(0x134);
const WHITE: u32 = 130;
const GREEN: u32 = 48;
// Retained prior-build mapping; new-build captures confirmed only white/green.
const RED: u32 = 99;
const UNKNOWN_GRACE: Duration = Duration::from_millis(500);

pub struct ApbCursorSource {
    reader: AtlasReader,
    pid: u32,
    base: EncodedAddress,
    process: HANDLE,
    window: windows::Win32::Foundation::HWND,
    waiting_for_pointer: bool,
    unknown_since: Option<(u32, Instant)>,
    trigger_suspended: bool,
    hit_previous: Option<(EncodedAddress, EncodedAddress, f32)>,
}

impl ApbCursorSource {
    pub fn connect(pid: u32) -> Result<Self, String> {
        let reader = AtlasReader::connect()?;
        let base = module_base_and_identity(&reader, pid)?;
        let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }
            .map_err(|error| format!("Open APB process for cursor monitoring failed: {error}"))?;
        let window = unsafe { FindWindowW(None, w!("APB Reloaded")) };
        if window.0 == 0 {
            unsafe {
                let _ = CloseHandle(process);
            }
            return Err(
                "APB Reloaded window was not found. Open APB and retry after its game window appears.".to_string(),
            );
        }
        let mut window_pid = 0u32;
        unsafe { GetWindowThreadProcessId(window, Some(&mut window_pid)) };
        if window_pid != pid {
            unsafe {
                let _ = CloseHandle(process);
            }
            return Err(format!(
                "APB window belongs to PID {window_pid}, expected {pid}."
            ));
        }
        let mut source = Self {
            reader,
            pid,
            base: EncodedAddress::new(base)?,
            process,
            window,
            waiting_for_pointer: false,
            unknown_since: None,
            trigger_suspended: false,
            hit_previous: None,
        };
        match source.read_pointer()? {
            Some(object) => {
                source.read_value(object)?;
            }
            None => source.report_waiting(),
        }
        Ok(source)
    }

    fn read_pointer(&self) -> Result<Option<u64>, String> {
        if unsafe { WaitForSingleObject(self.process, 0) } != WAIT_TIMEOUT {
            return Err("APB process exited while reading cursor state.".to_string());
        }
        let mut pointer_bytes = [0u8; 8];
        self.reader
            .read_exact(self.pid, self.base.get() + CURSOR_POINTER_RVA.get(), &mut pointer_bytes)?;
        let object = u64::from_le_bytes(pointer_bytes);
        if object == 0 {
            return Ok(None);
        }
        if !(0x10000..0x800000000000).contains(&object) || object % 8 != 0 {
            return Err(format!(
                "APB cursor pointer is invalid: 0x{object:X}."
            ));
        }
        Ok(Some(object))
    }

    fn read_value(&self, object: u64) -> Result<u32, String> {
        let mut value_bytes = [0u8; 4];
        self.reader
            .read_exact(self.pid, object + CURSOR_FIELD_OFFSET.get(), &mut value_bytes)?;
        Ok(u32::from_le_bytes(value_bytes))
    }

    fn report_waiting(&mut self) {
        if !self.waiting_for_pointer {
            crate::log_info!(
                "Memory pegasus waiting for APB district cursor pointer; shots are disabled until it appears."
            );
            self.waiting_for_pointer = true;
        }
    }

    pub fn reset_hit_event(&mut self) { self.hit_previous = None; }

    pub fn red_hit_event(&mut self) -> Result<bool, String> {
        let pointer = self.read_pointer()?;
        if unsafe { GetForegroundWindow() } != self.window {
            self.hit_previous = None;
            return Ok(false);
        }
        let Some(object) = pointer else {
            self.hit_previous = None;
            self.report_waiting();
            return Ok(false);
        };
        let mut header = [0u8; 8];
        let mut values = [0u8; 20];
        let mut after = [0u8; 8];
        self.reader.read_exact(self.pid, object, &mut header)?;
        self.reader.read_exact(self.pid, object + const { EncodedOffset::new(0x14c) }.get(), &mut values)?;
        self.reader.read_exact(self.pid, object, &mut after)?;
        if self.read_pointer()? != Some(object) || after != header {
            self.hit_previous = None;
            crate::log_info!("Hit sound cursor binding changed during sample; discarded.");
            return Ok(false);
        }
        let floats: [f32; 5] = std::array::from_fn(|index|
            f32::from_le_bytes(values[index*4..index*4+4].try_into().unwrap()));
        let timer = floats[0];
        if !floats.iter().all(|value| value.is_finite()) || !(-1.0..=1.1).contains(&timer) {
            return Err("Hit sound memory values are invalid. Restart processing; inspect verux_apb.log if this persists.".into());
        }
        let fingerprint = u64::from_le_bytes(header);
        let previous = self.hit_previous.replace((EncodedAddress::new(object)?, EncodedAddress::new(fingerprint)?, timer));
        let Some((old_object, old_header, old_timer)) = previous else { return Ok(false); };
        if old_object.get() != object || old_header.get() != fingerprint || timer <= 0.0 || timer <= old_timer + 0.05 {
            return Ok(false);
        }
        let matches = |rgba: [f32; 4]| floats[1..].iter().zip(rgba).all(|(a, b)| (*a-b).abs() < 0.001);
        if matches([1.0, 1.0, 1.0, 1.0]) { return Ok(false); }
        if !matches([1.0, 0.0, 0.0, 1.0]) {
            return Err(format!("Hit sound marker color is unclassified: {:?}. Processing stopped; inspect verux_apb.log.", &floats[1..]));
        }
        Ok(unsafe { GetForegroundWindow() } == self.window)
    }

    pub fn trigger_suspended(&self) -> bool { self.trigger_suspended }

    /// `None` cancels pending shots when focus, binding, or cursor signal is unavailable.
    pub fn should_fire(&mut self, on_red: bool, on_green: bool) -> Result<Option<bool>, String> {
        let object = match self.read_pointer()? {
            Some(object) => object,
            None => {
                self.report_waiting();
                self.unknown_since = None;
                return Ok(None);
            }
        };
        if self.waiting_for_pointer {
            crate::log_info!(
                "Memory pegasus cursor pointer ready; EchoDriver cursor reads resumed."
            );
            self.waiting_for_pointer = false;
        }
        if unsafe { GetForegroundWindow() } != self.window {
            self.unknown_since = None;
            return Ok(None);
        }
        let value = self.read_value(object)?;
        if self.read_pointer()? != Some(object) {
            self.unknown_since = None;
            return Ok(None);
        }
        match value {
            0 => {
                // Observed with HUD hidden during ADS; never classify it as a target.
                self.unknown_since = None;
                self.trigger_suspended = true;
                Ok(None)
            }
            RED => {
                self.unknown_since = None;
                self.trigger_suspended = false;
                Ok(Some(on_red))
            }
            GREEN => {
                self.unknown_since = None;
                self.trigger_suspended = false;
                Ok(Some(on_green))
            }
            WHITE => {
                self.unknown_since = None;
                self.trigger_suspended = false;
                Ok(Some(false))
            }
            other => {
                let now = Instant::now();
                match self.unknown_since {
                    Some((previous, since)) if previous == other => {
                        if now.duration_since(since) >= UNKNOWN_GRACE {
                            Err(format!(
                                "APB cursor value {other} (0x{other:X}) is not classified for this build. Pegasus stopped; inspect verux_apb.log."
                            ))
                        } else {
                            Ok(Some(false))
                        }
                    }
                    _ => {
                        crate::log_err!(
                            "APB cursor value {other} (0x{other:X}) is unclassified; pegasus will not fire while it is present."
                        );
                        self.unknown_since = Some((other, now));
                        Ok(Some(false))
                    }
                }
            }
        }
    }
}

impl Drop for ApbCursorSource {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.process);
        }
    }
}
