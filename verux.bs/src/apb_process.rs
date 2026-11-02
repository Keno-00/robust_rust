//! Explicit APB.exe process discovery for the Misc tab.

use windows::core::HRESULT;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_NO_MORE_FILES, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
};

const APB_EXE: &str = "APB.exe";

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Find the sole APB.exe process and verify that its process handle opens.
pub fn find_running_apb_pid() -> Result<u32, String> {
    let snapshot = OwnedHandle(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
            .map_err(|error| format!("Unable to enumerate processes: {error}"))?,
    );
    let mut entry = PROCESSENTRY32W::default();
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    let mut verified = Vec::new();
    let mut inaccessible = Vec::new();
    let mut next = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    loop {
        match next {
            Ok(()) => {
                let name_length = entry
                    .szExeFile
                    .iter()
                    .position(|character| *character == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..name_length]);
                if name.eq_ignore_ascii_case(APB_EXE) {
                    let pid = entry.th32ProcessID;
                    match unsafe {
                        OpenProcess(
                            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                            false,
                            pid,
                        )
                    } {
                        Ok(handle) => {
                            let _handle = OwnedHandle(handle);
                            match unsafe { WaitForSingleObject(handle, 0) } {
                                WAIT_TIMEOUT => verified.push(pid),
                                WAIT_OBJECT_0 => {
                                    inaccessible.push(format!("PID {pid}: process exited"))
                                }
                                other => inaccessible.push(format!(
                                    "PID {pid}: process status check failed ({other:?}, {})",
                                    windows::core::Error::from_win32()
                                )),
                            }
                        }
                        Err(error) => inaccessible.push(format!("PID {pid}: {error}")),
                    }
                }
                next = unsafe { Process32NextW(snapshot.0, &mut entry) };
            }
            Err(error) if error.code() == HRESULT::from_win32(ERROR_NO_MORE_FILES.0) => break,
            Err(error) => return Err(format!("Process enumeration failed: {error}")),
        }
    }

    if !inaccessible.is_empty() {
        return Err(format!(
            "APB.exe was found, but its process handle could not be opened ({})",
            inaccessible.join(", ")
        ));
    }
    match verified.as_slice() {
        [pid] => Ok(*pid),
        [] => Err("APB.exe is not running.".to_string()),
        _ => Err(format!(
            "Multiple APB.exe processes found (PIDs: {}). Close the extra instances and refresh again.",
            verified.iter().map(u32::to_string).collect::<Vec<_>>().join(", ")
        )),
    }
}
