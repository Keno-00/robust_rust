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

## Current driver assessment (2026-09-30)

The local `..\drayebr` KMDF driver accepts read-only process-memory requests of
1â€“4096 bytes, rejects invalid ranges, and returns exact bytes or an error. Its
device SDDL permits SYSTEM and Administrators. The separate echo ReadFile and
WriteFile endpoints only access the driver's own RAM buffer. APB cursor reads
were checked through EchoDriver after game restarts and matched the player's
white, green, and red observations. This supports local development use for
the cursor source; it does not establish broad deployment reliability.

The current service is demand-start and points at
`..\drayebr\build-rpm\EchoDriver.sys`. That binary is signed by a local
`WDKTestCert`, and the repository's documented setup uses Windows test mode.
It is not a production-signed driver package. The service path is tied to this
development checkout. A copy beside `verux_apb.exe` would not install the
service or make the test signature trusted on other Windows machines.

Before bundling for other users, the driver needs a production signing and
installation path, an update/uninstall flow, and validation on supported
Windows configurations. Review whether the educational echo endpoints and
general administrator process-read interface should be narrowed for the
product. Keep the driver as a separate signed `.sys` installed by an elevated
installer; the application can then use the Misc button to start and verify it.

Read-only local checks:

```powershell
sc.exe qc EchoDriver
Get-Service EchoDriver
Get-AuthenticodeSignature ..\drayebr\build-rpm\EchoDriver.sys
Get-FileHash ..\drayebr\build-rpm\EchoDriver.sys -Algorithm SHA256
```

Windows references: [starting a service](https://learn.microsoft.com/en-us/windows/win32/services/starting-a-service),
[driver signing](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/windows-driver-signing-tutorial),
and [non-PnP driver installation](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/installing-a-test-signed-catalog-file-for-a-non-pnp-driver).
