# EchoDriver Setup

This is for External Memory reading in APB. If this is too intrusive for you, you may choose Computer Vision in Aimbot Source and Triggerbot Source instead.

## Requirements

- 64-bit Windows
- Secure Boot Off
- Driver Test Signing

## 1. Disable Secure Boot

Before loading the kernel driver, you must disable Secure Boot.

1. Restart your computer and enter its BIOS/UEFI settings. The key varies by manufacturer, but is commonly Delete, F2, or Esc.
2. Locate Secure Boot, usually under the Boot or Security tab.
3. Set it to Disabled.
4. Save your changes and restart into Windows.

If BitLocker or Device Encryption is enabled, have your recovery key available before changing these settings.

## 2. Enable Driver Test Signing

Once Secure Boot is disabled, enable Windows test-signing mode.

1. Open Command Prompt as Administrator.
2. Enter the following command:

```
bcdedit /set testsigning on
```

3. After the command completes successfully, restart your computer.

Windows may display a Test Mode watermark on the desktop after restarting. Test-signing mode allows appropriately test-signed drivers to load. For our case, this allows EchoDriver to be installed.

## 3. Install the Kernel Driver

With the Verux launcher, install VERUX.APB and the launcher installs EchoDriver.

Without it, double-click **Install.cmd** and approve the administrator prompt. Wait for the installation message.

## 4. Start EchoDriver

1. Open **verux.apb.exe** and go to the Memory section.
2. Select Start EchoDriver.
3. Wait for EchoDriver to say Ready.

If setup fails, read the displayed error and `C:\ProgramData\VeruxAPB\Logs\EchoDriver-setup.log` (standalone) or `EchoDriver-migration.log` (launcher).

## Upgrade and Uninstall

To upgrade from 2.0, close VERUX.APB, run **Uninstall.cmd** from this folder, then run **Install.cmd**. If Windows reports the service is marked for deletion, restart Windows before installing.

To uninstall, close VERUX.APB, double-click **Uninstall.cmd**, and approve the administrator prompt. The test certificate, Secure Boot and test-signing settings remain unchanged.

## Release validation

Package source: `drayebr/build-crypto/EchoDriver.sys` and matching `EchoDriver.cer`. Only the public certificate is distributed. Setup pins both SHA-256 digests and the signer thumbprint. Renewing or changing the artifacts requires explicitly updating the pins.

Installation, fresh-machine trust, kernel loading, application verification and uninstall still require qualification on a supported Windows machine. Existing development services at other paths are rejected; setup never silently replaces them or restores another driver.
