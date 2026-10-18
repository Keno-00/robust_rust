# EchoDriver encrypted transport (v3)

Both process-read requests (PID, size, address) and response bytes use
AES-256-GCM. The driver authenticates before accessing a process; readers
authenticate before publishing plaintext. The IOCTL identifier, session
handshake, frame lengths and timing remain visible. No v2, XOR/rotation or
plaintext fallback is supported. Echo ReadFile/WriteFile remain independent
sample endpoints and cannot read another process.

## Keys and sessions

The installation key is a 32-byte REG_BINARY value at
`HKLM\SYSTEM\CurrentControlSet\Services\EchoDriver\Parameters\TransportKey`.
`drayebr/scripts/initialize-transport-key.ps1` first applies a
SYSTEM/Administrators-only ACL, then generates an OS RNG key. It preserves an
existing valid key. Missing/invalid keys prevent initialization; no key is
embedded in binaries, committed or logged.

The driver generates a random 16-byte ID per file handle and derives its key:

`HMAC-SHA256(TransportKey, ASCII("EchoDriver-v3-session") || 0x00 || session_id)`

The nonce is `direction:u32 || sequence:u64`, little-endian. Requests use
direction 1; responses use direction 2. The driver rejects zero, reused or
decreasing sequences, incorrect nonces, and sequences above `0xFFFFFF`.
Close and reconnect at the session limit. Authenticated requests consume their
sequence even when the requested memory read fails. Session queries are
idempotent and never reset the sequence. Responses must match the expected
session, sequence, direction and size, and pass GCM authentication.

File cleanup destroys the CNG session key; device cleanup wipes the installation
key. Mutable key/plaintext scratch buffers are cleared and Rust enables AES key
schedule zeroization. Python creates short-lived immutable byte strings whose
removal cannot guarantee memory erasure.

Administrators and SYSTEM can access the installation key. This protects the
transport against observers lacking that key; it does not protect against a
privileged debugger or compromised kernel. Existing device ACLs and user-address
validation remain necessary. Generate separate installation keys per machine.
For explicit rotation, stop the driver, close all readers, remove only
TransportKey, provision again and restart.

## Wire format

All integers are little-endian. C structure sizes have compile-time checks.
Session IOCTL `0x226004` (function `0x801`, METHOD_BUFFERED, FILE_READ_ACCESS)
takes no input and returns 32 bytes:

| Offset | Field |
| --- | --- |
| 0 | Version u32 = 3 |
| 4 | Reserved u32 = 0 |
| 8 | Session ID, 16 bytes |
| 24 | Last authenticated sequence u64 |

Read IOCTL remains `0x226002` (function `0x800`, METHOD_OUT_DIRECT,
FILE_READ_ACCESS). The complete 48-byte header is GCM associated data:

| Offset | Field |
| --- | --- |
| 0 | Version u32 = 3 |
| 4 | Direction u32 |
| 8 | Session ID, 16 bytes |
| 24 | Sequence u64 |
| 32 | Nonce, 12 bytes |
| 44 | Ciphertext/plaintext length u32 |
| 48 | GCM tag, 16 bytes |
| 64 | Ciphertext |

Request plaintext is `process_id:u32, size:u32, address:u64` (16 bytes), making
an 80-byte frame. Response ciphertext contains exactly the requested 1-4096
bytes. The driver checks the authenticated user-address range and exact buffer
lengths. Driver failures produce diagnostic debugger messages; callers return
actionable errors through existing application error handling. Rust clears the
caller's output before validation and any read attempt.

## Build and activate

From robust_rust, build in a regular shell:

```powershell
cargo build --bin verux_apb
& 'C:\Program Files\Microsoft Visual Studio\2022\Community\MSBuild\Current\Bin\MSBuild.exe' ..\drayebr\EchoDriver.sln /p:Configuration=Debug /p:Platform=x64 /p:OutDir=C:\Users\Admin\Documents\source_repos\drayebr\build-crypto\ /verbosity:minimal
```

Close all readers, then run from robust_rust in **Administrator PowerShell**:

```powershell
$ErrorActionPreference = 'Stop'
$svc = Get-Service EchoDriver
if ($svc.Status -ne 'Stopped') {
    Stop-Service EchoDriver
    $svc.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(15))
}
& ..\drayebr\scripts\initialize-transport-key.ps1
& sc.exe config EchoDriver binPath= 'C:\Users\Admin\Documents\source_repos\drayebr\build-crypto\EchoDriver.sys'
if ($LASTEXITCODE -ne 0) { throw 'EchoDriver service configuration failed' }
Start-Service EchoDriver
(Get-Service EchoDriver).WaitForStatus('Running', [TimeSpan]::FromSeconds(15))
```

The development `.sys` is test-signed; existing certificate/trust requirements
still apply. Do not use legacy `activate_read_driver.ps1` for this migration:
it can roll back to v2, which updated callers explicitly reject.

Launch the newly built Verux application and use its existing Start EchoDriver
control to validate a self-process read through the Rust transport. The C++
diagnostic client supports `build-crypto\EchoClient.exe --read <pid> <hex-address>
<count>`. Python callers retain the existing `read_exact` API. Use an address
owned by a diagnostic process with known bytes for comparison.

## Validation status (2026-10-07)

The Rust application build and Debug driver/C++ solution build passed. The Python
module imports; its CNG authenticated-mode structure is 88 bytes on x64.
Live v3 driver validation remains unverified: the execution shell is not
elevated and EchoDriver was stopped. No service configuration or installation
key was changed during implementation. No automated tests, fixtures or
test-only dependencies were added.

References: [Microsoft CNG features](https://learn.microsoft.com/en-us/windows/win32/seccng/cng-features),
[CNG programming](https://learn.microsoft.com/en-us/windows/win32/seccng/typical-cng-programming).
