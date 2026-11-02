# EchoDriver service integration and release status

## Misc control

The **Start EchoDriver** button starts the existing Windows `EchoDriver`
service. Verux already requests administrator elevation at launch. The button
queries the service, starts it if stopped, waits up to 15 seconds for Running,
then reads a known value from its own process through `\\.\EchoDevice`. This
confirms the installed driver supports the version 3 encrypted process-read protocol, not just that
Windows reports a running service. The result appears beside the button and in
`Documents\VeruxAPB\verux_apb.log`. A missing service, blocked load, device
error, or failed read is reported without selecting another capture source.
The button does not install, replace, or reconfigure a driver service.

Version 3 encrypts and authenticates both process-read requests and responses
with AES-256-GCM. Authentication precedes process access and plaintext delivery.
There is no v2, obfuscation or plaintext fallback. See
[encrypted transport](ECHO_ENCRYPTED_TRANSPORT.md) for key provisioning, session
lifecycle, build/loading commands and validation limitations.

## Installation

The launcher MSI installs the signed driver to `Program Files\VeruxAPB\Driver\EchoDriver.sys` through the compiled helper `packaging/echodriver/DriverInstaller.cs`. The standalone package uses `Setup.ps1`. Both pin the driver and certificate hashes and the signer thumbprint, and reject a service owned by another path.

The driver is test-signed with a public development certificate, so Windows needs Secure Boot off and test signing on, then a restart. The helper checks both before installing and stops with the steps from the user manual (`User Guide and Setup.pdf`, 3.1 and 3.2). `Preflight` reads the state without changing anything. `EnableTestSigning` runs `bcdedit /set testsigning on` and only runs after the user confirms. Secure Boot is never changed by the product.

Read-only local checks:

```powershell
sc.exe qc EchoDriver
Get-Service EchoDriver
Get-AuthenticodeSignature <installed EchoDriver.sys>
Get-FileHash <installed EchoDriver.sys> -Algorithm SHA256
```

Windows references: [starting a service](https://learn.microsoft.com/en-us/windows/win32/services/starting-a-service),
[driver signing](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/windows-driver-signing-tutorial),
and [non-PnP driver installation](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/installing-a-test-signed-catalog-file-for-a-non-pnp-driver).
