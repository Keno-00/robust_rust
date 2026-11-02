# Operator secrets ledger

Every key, credential and private backup that Verux operations depend on. Paths and purposes only; never put secret values in this file or any repository.

Location: `C:\Users\Admin\Desktop\deploy\Verux Secrets` (the "secrets folder"). Operator scripts find it through `license-server/scripts/VeruxSecrets.ps1`; set `VERUX_SECRETS_ROOT` to use another folder. Scripts fail with an error if the folder is missing. They never fall back to another location.

## Why this folder

Until 2026-11-02 these files lived under `%LOCALAPPDATA%`. Work done inside the Codex desktop app (a packaged Windows app) was silently redirected to `C:\Users\Admin\AppData\Local\Packages\OpenAI.Codex_2p2nqsd0c76g0\LocalCache\Local\`. The files were invisible to every other tool, and uninstalling or resetting Codex would have deleted them. The originals are still there; the secrets folder holds a hash-verified copy (2,247 files, `.pdb` debug symbols excluded).

## Inventory

| Item | Path in secrets folder | Used for | If lost |
|---|---|---|---|
| Release signing key (Ed25519, plain PEM) | `Verux\ReleasePublisher\release-ed25519.pem` | Signing launcher catalogs (`release.mjs prepare`, `Publish-Release.ps1`). Public half `20qEEX…7qg=` is pinned in every installed launcher. | No new APB or launcher version can reach users. Replacing it forces every user to reinstall the launcher manually. |
| Release public key | `Verux\ReleasePublisher\release-ed25519.pem.public` | Launcher `scripts/Build.ps1 -PublicKeyFile` | Recoverable from the private key. |
| License response signing key (DPAPI) | `VeruxServerSecurity\verux-20261003.pkcs8.dpapi` | Key ID `verux-20261003`; created by `initialize-security.ps1` and uploaded as a license Worker secret | Rotation needs a client release with the new public key. |
| Surface encryption keys and catalogs (DPAPI) | `VeruxServerSecurity\surface-labels\` (and `verux.apb.cb\`) | `prepare-*-surface.ps1`, `publish-surface.ps1` | Encrypted surface catalogs cannot be regenerated. |
| Color edition artifacts | `VeruxServerSecurity\color-artifacts\` | `enroll-color-artifacts.ps1` | Rebuild from source. |
| License DB backup (2026-10-03) | `VeruxServerSecurity\backups\20261003-192318\` | Pre-migration recovery record | Historical only. |
| Leonardo firmware readback | `VeruxFirmwareBackups\20261004\` | Recovery evidence (docs/LEONARDO_SETUP.md) | Historical only. |
| R2 upload credentials | `C:\Users\Admin\Desktop\deploy\api token.txt` | `Publish-Release.ps1 -CredentialsFile` (looks for "Access Key ID" / "Secret Access Key" labels) | Create a new bucket-scoped R2 API token in the Cloudflare dashboard. |

**Open item:** the private key that signed `EchoDriver.sys` (certificate `EchoDriver.cer`, thumbprint `18537D3D…2065407`) is not in the secrets folder. It is probably in the Windows certificate store. Locate it, back it up into the secrets folder, and add a row above.

Copies held by Cloudflare (write-only, cannot be read back): `RELEASE_SIGNING_KEY_PKCS8` on the `verux-catalog-renewal` Worker, and the response signing key on `verux-license`.

## Backup limits

- `.dpapi` files decrypt only under this Windows user on this PC. A Windows reinstall or a different account makes them unreadable, even from the backup.
- The Desktop copy is on the same disk as the originals. Keep a second copy of the secrets folder off this machine (USB drive or password manager), at minimum `release-ed25519.pem`.

## Rules for any person or AI agent

1. A new key, credential or private backup goes in the secrets folder and gets a row in this ledger in the same change.
2. Tell the operator in plain words when one is created, moved, rotated, uploaded to a service, or deleted.
3. Never store secrets under `%LOCALAPPDATA%`, `%APPDATA%`, a temp folder, or any app sandbox.
