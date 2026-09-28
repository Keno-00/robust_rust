//! Read-only connection to the EchoDriver process-read IOCTL.
//! Target discovery and projection are deliberately separate from this transport.

use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;

const MAX_READ_BYTES: usize = 4096;
// Must match drayebr/shared/ReadProtocol.h: CTL_CODE(FILE_DEVICE_UNKNOWN,
// 0x800, METHOD_OUT_DIRECT, FILE_READ_ACCESS).
const IOCTL_ECHO_READ_PROCESS: u32 = (0x22 << 16) | (1 << 14) | (0x800 << 2) | 2;

#[repr(C)]
struct ReadProcessRequest {
    process_id: u32,
    size: u32,
    address: u64,
}

pub struct MemoryReader {
    device: HANDLE,
}

impl MemoryReader {
    pub fn connect() -> Result<Self, String> {
        let device = unsafe {
            CreateFileW(
                w!("\\\\.\\EchoDevice"),
                GENERIC_READ.0,
                FILE_SHARE_READ,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|error| format!("Open EchoDevice failed: {error}. Check the driver service and administrator access."))?;
        Ok(Self { device })
    }

    pub fn read_exact(
        &self,
        process_id: u32,
        address: u64,
        output: &mut [u8],
    ) -> Result<(), String> {
        if process_id == 0 || output.is_empty() || output.len() > MAX_READ_BYTES {
            return Err(
                "Invalid memory read request: PID must be nonzero and length must be 1–4096 bytes."
                    .to_string(),
            );
        }
        let request = ReadProcessRequest {
            process_id,
            size: output.len() as u32,
            address,
        };
        let mut returned = 0u32;
        let read_result = unsafe {
            DeviceIoControl(
                self.device,
                IOCTL_ECHO_READ_PROCESS,
                Some(&request as *const _ as *const std::ffi::c_void),
                std::mem::size_of::<ReadProcessRequest>() as u32,
                Some(output.as_mut_ptr() as *mut std::ffi::c_void),
                output.len() as u32,
                Some(&mut returned),
                None,
            )
        };
        if let Err(error) = read_result {
            output.fill(0);
            return Err(format!(
                "EchoDriver read failed for PID {process_id} at 0x{address:X}: {error}"
            ));
        }
        if returned != request.size {
            output.fill(0);
            return Err(format!(
                "EchoDriver returned {returned} of {} requested bytes at 0x{address:X}.",
                request.size
            ));
        }
        Ok(())
    }
}

impl Drop for MemoryReader {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.device);
        }
    }
}
