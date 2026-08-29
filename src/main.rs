#![windows_subsystem = "windows"]

use robust_rust::log_err;
use std::env;
use std::os::windows::ffi::OsStrExt;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK, SW_SHOW};

fn show_fatal_error(message: &str) {
    let title: Vec<u16> = std::ffi::OsStr::new("Verux APB")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let body = format!("{message}\n\nDetails were written to Documents\\VeruxAPB\\verux_apb.log.");
    let body: Vec<u16> = std::ffi::OsStr::new(&body)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(body.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn is_elevated() -> bool {
    unsafe {
        let mut token: HANDLE = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
        let res = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut std::ffi::c_void),
            size,
            &mut size,
        );

        let _ = CloseHandle(token);

        if res.is_err() {
            return false;
        }

        elevation.TokenIsElevated != 0
    }
}

/// Re-launch this executable elevated via ShellExecuteW "runas".
/// Passes the original user's LOCALAPPDATA path as --appdata so that
/// DPAPI-protected ledger files are always stored and read from the
/// standard (non-elevated) user's AppData directory.
fn run_as_admin(original_appdata: &str) -> bool {
    let current_exe = match env::current_exe() {
        Ok(path) => path,
        Err(_) => return false,
    };

    let mut exe_path: Vec<u16> = current_exe.as_os_str().encode_wide().collect();
    exe_path.push(0);

    // Forward all existing args, stripping any stale --appdata pairs first,
    // then append the freshly captured LOCALAPPDATA value.
    let existing_args: Vec<String> = env::args().skip(1).collect();
    let mut forwarded: Vec<String> = Vec::new();
    let mut skip_next = false;
    for arg in existing_args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--appdata" {
            skip_next = true;
            continue;
        }
        forwarded.push(arg);
    }

    // Append --appdata with the original (pre-elevation) LOCALAPPDATA.
    // Wrap in quotes to handle usernames that contain spaces.
    forwarded.push("--appdata".to_string());
    forwarded.push(format!("\"{}\"", original_appdata.replace('"', "")));

    let args_str = forwarded.join(" ");
    let mut args_wide: Vec<u16> = std::ffi::OsStr::new(&args_str).encode_wide().collect();
    args_wide.push(0);

    unsafe {
        let instance = ShellExecuteW(
            None,
            windows::core::w!("runas"),
            PCWSTR(exe_path.as_ptr()),
            PCWSTR(args_wide.as_ptr()),
            None,
            SW_SHOW,
        );
        instance.0 as usize > 32
    }
}

#[link(name = "winmm")]
extern "system" {
    fn timeBeginPeriod(period: u32) -> u32;
    fn timeEndPeriod(period: u32) -> u32;
}

fn main() {
    // --- Restore original user's LOCALAPPDATA before anything else ---
    // When relaunched as Admin via "runas", LOCALAPPDATA points to the
    // Administrator profile, not the regular user's profile. This breaks
    // DPAPI-protected ledger files (ledger.key / ledger.state) because
    // DPAPI encrypts under the calling user's master key.
    // We fix this by forwarding the pre-elevation LOCALAPPDATA as --appdata
    // and restoring it here so all license storage uses the correct path.
    let args: Vec<String> = env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--appdata") {
        if let Some(path) = args.get(pos + 1) {
            // Strip any surrounding quotes that were added for quoting in ShellExecuteW.
            let clean = path.trim_matches('"');
            if !clean.is_empty() {
                env::set_var("LOCALAPPDATA", clean);
            }
        }
    }

    if !is_elevated() {
        // Capture LOCALAPPDATA now, before the elevation boundary.
        let original_appdata = env::var("LOCALAPPDATA").unwrap_or_default();
        if run_as_admin(&original_appdata) {
            std::process::exit(0);
        } else {
            log_err!("Failed to relaunch as administrator.");
            show_fatal_error("Verux APB requires administrator permission to start.");
            std::process::exit(1);
        }
    }

    unsafe {
        let _ = timeBeginPeriod(1);
    }

    if let Err(error) = robust_rust::ui::run() {
        log_err!("failed to start {error}");
        show_fatal_error(&format!("Verux APB could not start: {error}"));
    }

    unsafe {
        let _ = timeEndPeriod(1);
    }
}
