# EchoDriver setup

Requires 64-bit Windows, Secure Boot disabled, and test-signing enabled followed by a restart. This package uses a public development test certificate, not Microsoft production signing.

Double-click **Install.cmd** and approve the Windows administrator prompt. Wait for the installation confirmation, then close the setup window.

Then open VERUX.APB, select Memory, and click **Start EchoDriver** to verify the interface. Keep the installation at its permanent location. If setup fails, read the displayed error and `C:\ProgramData\VeruxAPB\Logs\EchoDriver-setup.log`. Do not disable additional Windows protections to suppress errors. If PowerShell policy blocks the script, report that policy error.

To uninstall, close VERUX.APB, double-click **Uninstall.cmd**, and approve the Windows administrator prompt.

Uninstall removes only this package's service and driver file. The test certificate remains because other development drivers may share it. Test-signing and Secure Boot settings remain unchanged.

## Manual section 3.3

1. Open the provided **EchoDriver** folder.
2. Double-click **Install.cmd**.
3. Approve the administrator prompt and wait for installation to complete.
4. Open **verux.apb.exe**, go to **Memory**, and click **Start EchoDriver**.
5. Confirm that the driver passes verification.

## Release validation

Package source: `drayebr/build-rpm/EchoDriver.sys` and matching `EchoDriver.cer`. Only the public certificate is distributed. Setup pins both SHA-256 digests and the signer thumbprint. Renewing or changing the artifacts requires explicitly updating the pins.

Syntax and packaged hashes/signature can be checked without running setup. Installation, fresh-machine trust, kernel loading, application verification and uninstall still require qualification on a supported Windows machine. Existing development services at other paths are rejected; setup never silently replaces them or restores another driver.
