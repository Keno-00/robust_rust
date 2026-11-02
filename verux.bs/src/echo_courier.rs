//! Starts the installed EchoDriver service and verifies that its device opens.

use std::thread;
use std::time::{Duration, Instant};

use windows::core::{w, PCWSTR};
use windows::Win32::Security::SC_HANDLE;
use windows::Win32::System::Services::{
    CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, StartServiceW,
    SC_MANAGER_CONNECT, SC_STATUS_PROCESS_INFO, SERVICE_QUERY_STATUS, SERVICE_RUNNING,
    SERVICE_START, SERVICE_START_PENDING, SERVICE_STATUS_PROCESS, SERVICE_STOPPED,
};
use windows::Win32::System::Threading::GetCurrentProcessId;

use crate::atlas_reader::AtlasReader;

const START_TIMEOUT: Duration = Duration::from_secs(15);

struct ServiceHandle(SC_HANDLE);

impl Drop for ServiceHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseServiceHandle(self.0);
        }
    }
}

fn current_state(service: SC_HANDLE) -> Result<SERVICE_STATUS_PROCESS, String> {
    let mut status = SERVICE_STATUS_PROCESS::default();
    let buffer = unsafe {
        std::slice::from_raw_parts_mut(
            &mut status as *mut SERVICE_STATUS_PROCESS as *mut u8,
            std::mem::size_of::<SERVICE_STATUS_PROCESS>(),
        )
    };
    let mut needed = 0u32;
    unsafe { QueryServiceStatusEx(service, SC_STATUS_PROCESS_INFO, Some(buffer), &mut needed) }
        .map_err(|error| format!("Query EchoDriver status failed: {error}"))?;
    Ok(status)
}

fn verify_read_protocol() -> Result<(), String> {
    let reader = AtlasReader::connect().map_err(|error| {
        format!("EchoDriver is running, but its device cannot be opened: {error}")
    })?;
    let probe = 0x4543_484F_5245_4144u64;
    let mut observed = [0u8; 8];
    reader
        .read_exact(
            unsafe { GetCurrentProcessId() },
            &probe as *const u64 as u64,
            &mut observed,
        )
        .map_err(|error| {
            format!("EchoDriver is running, but its process-read interface failed: {error}")
        })?;
    if observed != probe.to_le_bytes() {
        return Err("EchoDriver is running, but its process-read interface returned incorrect bytes. Check the installed driver build.".to_string());
    }
    Ok(())
}

/// Read-only readiness check; never starts the service.
pub fn echodriver_status() -> Result<String, String> {
    let manager = ServiceHandle(
        unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT) }
            .map_err(|error| format!("Open Windows Service Control Manager failed: {error}"))?,
    );
    let service = ServiceHandle(
        unsafe { OpenServiceW(manager.0, w!("EchoDriver"), SERVICE_QUERY_STATUS) }
            .map_err(|error| format!("Open EchoDriver service failed: {error}. Check that the signed service is installed."))?,
    );
    let status = current_state(service.0)?;
    if status.dwCurrentState == SERVICE_STOPPED {
        return Ok("Stopped".into());
    }
    if status.dwCurrentState == SERVICE_START_PENDING {
        return Ok("Starting...".into());
    }
    if status.dwCurrentState != SERVICE_RUNNING {
        return Ok(format!("Service state {}", status.dwCurrentState.0));
    }
    verify_read_protocol()?;
    Ok("Ready".into())
}

pub fn start_echodriver() -> Result<String, String> {
    let manager = ServiceHandle(
        unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT) }
            .map_err(|error| format!("Open Windows Service Control Manager failed: {error}"))?,
    );
    let service = ServiceHandle(
        unsafe { OpenServiceW(manager.0, w!("EchoDriver"), SERVICE_QUERY_STATUS | SERVICE_START) }
            .map_err(|error| {
                format!(
                    "Open EchoDriver service failed: {error}. Check that the signed service is installed and Verux is elevated."
                )
            })?,
    );

    let initial = current_state(service.0)?;
    let already_running = initial.dwCurrentState == SERVICE_RUNNING;
    if !already_running && initial.dwCurrentState == SERVICE_STOPPED {
        unsafe { StartServiceW(service.0, None) }
            .map_err(|error| format!("Start EchoDriver service failed: {error}"))?;
    } else if !already_running && initial.dwCurrentState != SERVICE_START_PENDING {
        return Err(format!(
            "EchoDriver is in state {} and cannot be started yet. Check Windows Services.",
            initial.dwCurrentState.0
        ));
    }

    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        let status = current_state(service.0)?;
        if status.dwCurrentState == SERVICE_RUNNING {
            verify_read_protocol()?;
            return Ok(if already_running {
                "EchoDriver is already running; its process-read interface passed verification."
                    .to_string()
            } else {
                "EchoDriver started; its process-read interface passed verification.".to_string()
            });
        }
        if status.dwCurrentState == SERVICE_STOPPED {
            return Err(format!(
                "EchoDriver stopped during startup (Win32 exit code {}, service exit code {}). Check Windows Event Viewer and the driver signature.",
                status.dwWin32ExitCode, status.dwServiceSpecificExitCode
            ));
        }
        if Instant::now() >= deadline {
            return Err("EchoDriver did not reach Running within 15 seconds. Check Windows Services and Event Viewer.".to_string());
        }
        thread::sleep(Duration::from_millis(200));
    }
}
