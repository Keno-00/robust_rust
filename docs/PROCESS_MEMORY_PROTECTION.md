# Process memory access boundary

The application applies a Windows process DACL after elevation and before product initialization, including ML command modes. Startup fails with the existing fatal dialog and a diagnostic log entry if installation fails.

The Owner Rights entry grants only PROCESS_QUERY_LIMITED_INFORMATION and SYNCHRONIZE (0x00101000). Its presence suppresses the owner's implicit WRITE_DAC access. SYSTEM retains full access. Ordinary external callers receive no memory read/write/operation, handle duplication, remote thread creation, termination, or DACL modification rights. This controls access to the whole process address space without changing individual allocations or page permissions.

This is a bounded access control layer, not complete memory confidentiality. It does not revoke handles opened before installation, protect thread objects or inherited handles, block administrators using SeDebugPrivilege, restrict SYSTEM/kernel drivers, prevent in-process compromise, or encrypt active data. The startup interval before installation remains exposed. Existing secret zeroization and signed license verification remain necessary. Wasmtime requires executable generated code, so a blanket dynamic-code prohibition would break the selected runtime.

Validation: `cargo check --bin verux_apb --locked`. On Windows, validate the same DACL in a disposable process: limited-query OpenProcess must succeed while PROCESS_VM_READ, PROCESS_VM_WRITE, PROCESS_VM_OPERATION, PROCESS_CREATE_THREAD, PROCESS_DUP_HANDLE, and WRITE_DAC opens fail with ERROR_ACCESS_DENIED under a token without SeDebugPrivilege. Full application startup and capture/ML behavior still require a manual run; a compile check alone does not establish runtime compatibility.

Windows API reference: https://learn.microsoft.com/en-us/windows/win32/procthread/process-security-and-access-rights

Validation recorded for this change: the locked application compile check passed. A disposable Windows process applied the same descriptor successfully; limited query (0x1000) succeeded, and VM_READ (0x10), VM_WRITE (0x20), VM_OPERATION (0x8), CREATE_THREAD (0x2), DUP_HANDLE (0x40), and WRITE_DAC (0x40000) were each denied with Windows error 5. The full application was not launched during this validation.
