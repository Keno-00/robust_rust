//! Process-wide access control. This is not protection from privileged or
//! kernel attackers, existing handles, or code executing inside this process.
use windows::core::w;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{
    SetKernelObjectSecurity, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
};
use windows::Win32::System::Threading::GetCurrentProcess;

/// Restrict new external process handles before initializing product features.
/// Owner Rights explicitly removes the owner's implicit WRITE_DAC grant.
/// The owner retains limited query and synchronize; SYSTEM retains full access
/// for Windows services. No ordinary account receives VM access, duplication,
/// remote thread creation, termination, or permission-changing rights.
pub fn initialize() -> Result<(), String> {
    unsafe {
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            w!("D:(A;;0x00101000;;;OW)(A;;GA;;;SY)"),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
        .map_err(|error| failure("create_process_dacl", error))?;

        let result =
            SetKernelObjectSecurity(GetCurrentProcess(), DACL_SECURITY_INFORMATION, descriptor);
        // LocalFree succeeds when it returns NULL. The windows 0.52 binding
        // models that sentinel as an error, so a zero error code is success.
        let released = LocalFree(HLOCAL(descriptor.0));
        result.map_err(|error| failure("apply_process_dacl", error))?;
        if let Err(error) = released {
            if error.code().0 != 0 {
                return Err(failure("release_process_dacl", error));
            }
        }
    }
    crate::log_info!("Memory guard active: process DACL restricts external memory and injection access; SYSTEM and privileged access are outside this boundary.");
    Ok(())
}

fn failure(stage: &str, error: windows::core::Error) -> String {
    crate::log_err!("Memory guard failed: stage={}, error={}", stage, error);
    "Memory protection could not be enabled. Restart the application; if this persists, contact support with the diagnostic log.".into()
}
