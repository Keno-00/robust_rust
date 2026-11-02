# Launcher release workflow

## Current qualified release (2026-10-19)

Launcher 0.3.0 is published in signed catalog revision 13, alongside the unchanged APB 2.1.3 installer. The authoritative Worker deployment for the library/access endpoints is `26aa9288-c12a-4b73-ace7-c139ca1fc41a`. Library metadata is live with no notices. Migration 0005 was applied and recorded individually; the pre-existing 0004 migration status was left untouched.

Version 0.3.0 live checks passed: automatic detection of this account's existing valid protected APB key; authenticated library response; paid download 200 with a valid signed grant, 403 without it or with a modified grant; public launcher download 200; no metered product session created by launcher verification. A user-supplied real expired key produced authenticated usage_expired status; the actual GUI automatically uninstalled APB and its managed driver with MSI exit 0, preserved the profile directory, retained the EXPIRED card/banner, and exposed RENEW and the copied LICENSE popup without install/launch actions. The original protected working key was restored byte-for-byte and APB was reinstalled successfully through the licensed launcher flow; its client hash matched the release and EchoDriver was running afterward. No production license entitlement was altered to manufacture an expiry.

The empty-notice behavior and product modal were visually checked. Non-empty notice stacking and the actual passage of five days have not been exercised; no fake customer announcements or clock changes were introduced. Earlier reboot/interruption qualification limits below still apply.

The user completed the actual installed-launcher UPDATE flow from 0.2.1 to 0.3.0 and confirmed automatic reopening. Independent verification confirmed the installed 0.3.0 executable matches the published signed manifest; APB is restored with its driver running. The working protected license was verified byte-for-byte before temporary encrypted qualification backups were removed. The shared license ledger and normal product usage session flow were not replaced.

Publication exposed a real transport issue after the renewal Worker began writing JSON metadata: automatic gzip changed the S3 GET ETag into a weak validator, causing conditional publication to fail with 412 even when the revision had not changed. The publisher now requests identity encoding and explicitly rejects weak ETags. Revision 13 committed successfully with the original concurrency protection intact; no conditional check was removed and no released bytes were overwritten.

Catalog revision 7 established launcher 0.2.1 and APB installer 2.1.3. Scheduled metadata renewal increments revisions without changing these packages. The APB client and driver binaries are unchanged; 2.1.3 is the installer revision. This section supersedes earlier deployment notes below.

Actual operations on this Windows PC passed: migration from the existing development service, repair, uninstall, fresh installation, and launcher self-update from 0.2.0 to 0.2.1 with automatic reopening. The installed launcher matches the release executable and EchoDriver is running from the managed installation. A failed installation also restored the recorded development service and its running state. Reboot and interrupted-installation scenarios still require qualification before broad customer rollout.

The MSI now uses a compiled native installer helper for the existing driver flow. It does not execute the packaged PowerShell scripts or change Windows execution policy. Service ImagePath uses the Windows kernel path form. Repair is the customer-facing label for incomplete or damaged installations.

## Automatic catalog renewal

The separate `verux-catalog-renewal` Worker runs every four hours at minute 17 UTC: 00:17, 04:17, 08:17, 12:17, 16:17, 20:17 Manila time. Deployment configuration: `license-server/wrangler.renewal.toml`. It has no HTTP handler, workers.dev URL, preview URL, or public route. No PC needs to remain online.

It verifies the current release signature, preserves all product/package metadata and bytes, advances revision/timestamps, signs the renewed catalog, archives it under `catalog/renewals/`, and conditionally replaces the current catalog only if its ETag is unchanged. Publication races fail visibly and retain the newer publication. The scheduled renewal accepts lifetimes of one to seven days; the current catalog lifetime is one day. Very recent publications are skipped to make duplicate scheduling harmless. Failures appear as `CATALOG_RENEWAL_FAILED` in Cloudflare logs.

The secret `RELEASE_SIGNING_KEY_PKCS8` is the existing release key in base64 PKCS#8 DER, stored only on the private renewal Worker; the public delivery/license Worker does not receive it. It is separate from the licensing response key. This is an operational separation, not cryptographic delegation: compromise of this key still grants release-signing authority. The reviewed renewal code itself changes only revision and timestamps. Do not add a public invocation endpoint.

Discord failure delivery uses the optional secret `RENEWAL_DISCORD_WEBHOOK_URL`, restricted to an HTTPS discord.com webhook. Provision it from an operator-owned private file; never commit or print its value. No success messages are sent. Delivery is best effort: a killed Worker or a schedule that never executes cannot send its own failure alert. Cloudflare logs remain authoritative; independent monitoring is needed to detect absence of execution.

Deployment qualification on October 19: an actual Cloudflare scheduled invocation renewed revision 7 to 8 successfully (3 ms CPU). The installed 0.2.1 launcher accepted the renewed signature; product/package metadata was byte-for-byte equivalent after excluding revision/timestamps. A duplicate invocation skipped the recent publication. The temporary minute schedule was replaced with the four-hour production schedule. Both signing and Discord secrets were provisioned. No deliberate renewal failure or Discord test message was triggered.

Deploy or inspect the private Worker from `license-server`:

```text
npx wrangler deploy --config wrangler.renewal.toml
npx wrangler secret list --config wrangler.renewal.toml
npx wrangler tail verux-catalog-renewal --config wrangler.renewal.toml --format json
```

Software publication remains an operator action through `Publish-Release.ps1`; the renewal Worker cannot choose a new product, package, artwork, or version through its scheduled code. Before preparing a new release, read and verify the live catalog and choose a strictly higher revision. A prepared folder becomes stale after renewal; the publisher deliberately rejects stale revisions rather than rebasing a signed release silently. If a renewal wins a publication race, prepare/review/sign a fresh higher revision and retry.

An expired catalog still fails visibly; it is not silently replaced with cached metadata. Keep the local private key backed up outside repositories for future software publication. The old manual `-RenewInto` operation remains available only when its input exactly matches the current published catalog.

## Product access (current behavior)

Launcher 0.3.0 uses the signed `/v2/library/policy` response alongside the unchanged signed release catalog. Free products appear immediately; a + grid card opens the copied APB License Key / Submit modal. Paid products require a verified saved or newly entered key. The launcher reads existing product DPAPI storage (and validates a legacy login.ini before migrating it in the GUI); it does not create a product usage session. APB's existing runtime licensing remains authoritative. Possession of an MSI alone does not grant product use.

The authoritative server reuses activation validation/accounting under the existing per-license coordinator, returning a five-minute product-bound download grant instead of creating a session. Paid MSI routes require that grant and a currently active, unexhausted license; responses are private/no-store. Free launcher self-updates remain available. Unknown access policy fails visibly rather than defaulting to free. The public catalog still exposes product metadata; card hiding is a presentation rule, not metadata secrecy. The launcher revalidates before launch, download (including cached packages), and installation execution.

On confirmed usage expiry, the server records first confirmed expiry in `launcher_license_expiry` (migration 0005). The launcher attempts native MSI uninstall while running, preserving customer profiles and settings. It checks the key again before uninstall and refuses changed credentials or foreign driver ownership. It never interprets network/provider failures, invalid storage or local clock errors as expiry. Failures/cancellations are visible; an explicit CHECK retries removal, while background checks avoid repeating elevation prompts after a failed attempt in the same process. Restart handling uses the existing journal. The expired product card/banner remains for five days from the server's first confirmed expiry, with RENEW and LICENSE actions. A valid key permits installation again; after the deadline the card is hidden and + allows new enrollment. The launcher top banner is unaffected. There is no background service or uninstall while the launcher is closed.

Production library metadata is `license-server/releases/library.source.json`, stored privately at `library/current.json` in the release bucket. Every catalog product needs an explicit free/paid record. Paid metadata includes its purchase URL and compatible protected storage directory/entropy; the client has no APB-specific key path, purchase URL or access rule. APB uses the supplied https://payhip.com/b/J3Omz URL. Launcher self-update must remain free.

## Notice board operations

Library metadata contains `notices`, currently `[]`; an empty list displays nothing. A notice has `id` (unique stable lowercase ID), `product_id` (null for launcher startup, or the exact product ID), `title`, and plain-text `text`. Notices queue in array order, one modal at a time, with only OKAY. Startup notices appear once per launcher run. Product notices are fetched freshly on card click and precede the product modal. Notices contain no scripts, fields or executable actions; product renewal uses the separate purchase link.

Publish reviewed library/notice metadata without rebuilding applications or publishing a software release:

```powershell
.\license-server\scripts\Publish-Release.ps1 `
  -PreparedDirectory .\license-server\releases\library.source.json `
  -CredentialsFile 'C:\PRIVATE\api token.txt' -Library
```

The publisher validates field, size, product and notice limits and conditionally replaces the prior configuration using its ETag. Remove a notice from the array and republish to withdraw it. Do not put keys, customer data or internal documentation in notices. Routine catalog renewal does not edit library metadata or notices.

Launcher source: sibling folder ../verux_launcher. Authoritative license server: license-server. Licensing behavior is unchanged.

## Authoring and thumbnails

Edit license-server/releases/catalog.source.json. It begins with the supplied banner and no products; there are no placeholder products in customer code.

Root fields: revision (strictly increasing positive integer), ttl_seconds (1 to 604800; default 86400), banner (relative PNG path), products (array in display order), launcher (null or the launcher's own product record).

Product fields: id, title, requisites, thumbnail, banner, release.
- id: stable lowercase letters/digits/dots/hyphens; first character alphanumeric; max 64 bytes.
- title: max 80 UTF-8 bytes.
- requisites: max 16 short strings, each <=160 bytes. MSI launch conditions and structured Windows/architecture fields enforce actual prerequisites.
- thumbnail: relative PNG path, 16:9. Use the normal creative workflow to create artwork; 640x360 is a practical size.
- banner: relative PNG path, aspect ratio between 2:1 and 8:1.
- release fields below.

Release source fields:
- version: three-component MSI version; major/minor <=255, build <=65535.
- package: relative product MSI path, <=2 GiB.
- product_code: MSI ProductCode GUID in braces, new for a new major-upgrade package.
- upgrade_code: stable product-specific MSI UpgradeCode GUID in braces.
- directory: relative directory under 64-bit Program Files, matching MSI InstallLocation. No overlap with another catalog product.
- executable: relative EXE path inside the product directory.
- files: array of objects with path (relative installed path) and source (relative local file). Include client, driver, helpers and every immutable shipped file; exclude settings and generated secrets.
- min_windows_build: integer >=17763.
- architecture: x64.

Source files must be under the catalog source folder. Preparation rejects traversal/junction escapes and calculates all file/package hashes, sizes, product namespaces, timestamps, and executable digest. No hand-entered fake digests or secrets.

PNGs: <=8 MiB, width/height <=4096, <=8 million pixels each; client total decoded artwork budget 32 million pixels. The top banner retains the approved 2560:500 proportions. Publish real thumbnails; the runtime has no fallback product artwork.

## Packaging contract

A product release MSI includes its EXE plus driver/other required immutable files. The launcher does not accept arbitrary executable arguments, scripts, transforms, or installation commands from metadata.

The MSI must:
1. Be per-machine x64, with ALLUSERS=1 and matching ProductCode, UpgradeCode, ProductVersion.
2. Publish ARPINSTALLLOCATION matching the signed directory.
3. Preserve settings, licenses, and all existing security behavior.
4. Provide proper upgrade, repair, rollback, files-in-use, driver/service, and restart handling.
5. Have the exact payload bytes named in the signed installed-file manifest.
6. Keep the existing approved driver/certificate flow. The launcher imposes no official Windows certificate requirement and changes no OS security policy.
7. Reject conflicting existing driver-service ownership. Do not let independently owned product packages silently replace a shared service.
8. Be qualified in a disposable Windows environment before publication, especially driver failure, rollback, repair, and restart cases.

Native package inspection validates MSI identity, not the safety of every MSI custom action. Do not claim driver qualification from compilation alone. Existing APB driver scripts are preserved; missing EXE/driver binaries are not replaced with samples.

The launcher uses fixed passive MSI installation, no automatic restart, no automatic Restart Manager shutdown, verbose logs, and explicit repair for the same ProductCode. It validates installed file hashes before launch. Interrupted transactions require recovery; successful installs requiring restart block launch until a new Windows boot. Products commit separately, so no global multi-product rollback is claimed.

## Own-key release signing

This is your own release signature flow, separate from optional Windows Authenticode and from the license-server response key.

Initialize a new key only when needed, outside every repository:

    node license-server/scripts/release.mjs init C:/PRIVATE/release-ed25519.pem

The command refuses overwrite and writes a public companion file. Restrict the private key ACL to the release operator, back it up securely, and never paste/commit/embed its contents.

Private operator environment:
- VERUX_RELEASE_PRIVATE_KEY_FILE: private PEM path.
- VERUX_LAUNCHER_INSPECTOR: absolute path to the built launcher EXE.

Prepare a new directory and inspect the readable metadata and artwork:

    node license-server/scripts/release.mjs prepare license-server/releases/catalog.source.json C:/RELEASES/revision-N
    node license-server/scripts/release.mjs validate C:/RELEASES/revision-N/catalog.json

Build the launcher with its Build.ps1, the HTTPS origin, and the public companion file. Then use the built launcher's --verify-catalog command against the exact prepared envelope. Product package identity is inspected during preparation and again before installation.

Publish fresh, higher metadata revisions before expiry, even if binaries are unchanged. A signing-key replacement requires a planned installed-launcher trust-anchor migration. The tool refuses to replace a catalog signed by an unexpected key; changing a server setting does not change installed client pins.

## Server and publication

Provision private R2 bucket verux-release-artifacts before deploying its Wrangler binding. Never enable public bucket access. The Worker serves only the strict GET catalog/object paths, with limits and diagnostic failures. No public upload/delete/signing route exists. Existing licensing endpoints are unchanged.

Review the full server worktree before deployment; unrelated local server changes predate this launcher work.

Provide bucket-scoped R2 S3 credentials only to the private operator/CI environment:
- R2_ACCOUNT_ID
- R2_BUCKET
- R2_ACCESS_KEY_ID
- R2_SECRET_ACCESS_KEY

Use MFA and least privilege. Do not place these credentials or the release private key in the launcher or public Worker.

    node license-server/scripts/release.mjs publish C:/RELEASES/revision-N

Publication:
1. Verify prepared signature, metadata and all local artifact hashes.
2. Upload immutable product-scoped objects using atomic If-None-Match create-only writes; verify existing bytes instead of overwriting.
3. Archive the signed catalog at catalog/history/<revision>.json.
4. Replace catalog/current.json last using an ETag compare-and-swap.

Concurrent/stale publication fails visibly. Product objects live at objects/<product-id>/<sha256>.png or .msi; top branding uses namespace _shell. Private signing remains outside the Worker.

Client signature domain: ASCII VERUX-RELEASE-CATALOG-V1 followed by LF, then exact UTF-8 JSON payload. Envelope fields are base64 payload and signature. The client rejects expiry, future timestamps, revision rollback/reuse, unknown fields, cross-product assets, unsafe paths, wrong MSI identity, downgrade and changed installed files. No unsigned or offline fallback is used. First-install replay exposure is bounded by expiry; the launcher does not claim to protect local state from the machine owner.

## Reproducible checks

    cargo build --locked --manifest-path ../verux_launcher/Cargo.toml
    cargo fmt --manifest-path ../verux_launcher/Cargo.toml --check
    node --check license-server/scripts/release.mjs

From license-server:

    ./node_modules/.bin/tsc --noEmit
    ./node_modules/.bin/wrangler deploy --dry-run --outdir <new-local-output>

No automated tests, fixtures, or test-only dependencies were added. Live R2 publication and production package install/upgrade/repair/reboot require real artifacts and private provisioning; they are not claimed by source builds.

## Packaging the files normally collected for a release

The local New-ProductRelease.ps1 command accepts a built EXE and, for APB, the existing driver folder. WiX 4 is a build-time compiler only; no extra runtime service or official signing certificate is required. The command produces an embedded-cab MSI and product.source.json with stable product upgrade/component identities and a version-specific ProductCode.

    ./license-server/scripts/New-ProductRelease.ps1 -ProductId verux.apb -Title Verux.APB -Version 2.1.0 -ClientExe <built-EXE> -DriverFolder <driver-folder> -InstallDirectory VeruxAPB -OutputDirectory <new-release-folder> -WixPath <WiX-compiler>

The APB folder must contain the matching EchoDriver.sys, EchoDriver.cer, Setup.ps1 and Initialize-TransportKey.ps1. The adapter checks Setup.ps1 matches this repository and verifies all three pinned payload hashes before building. The scripts are copied unchanged. Original service-path ownership checks, public test-certificate validation/provisioning, protected transport-key generation and ACLs remain in that selected implementation. No security setting or execution-policy bypass is added.

MSI checks conflicting service ownership before mutation, stops/services through native ServiceControl, and uses the existing Setup.ps1. Initial-service rollback calls the existing uninstall action. A major upgrade stops the existing service without deleting its transport-key registry data; standalone uninstall uses the original uninstall action, with the original install action scheduled for rollback. Certificate retention matches the existing setup's uninstall behavior. Build/identity validation does not replace live rollback qualification.

Thumbnail and banner inputs are optional at the package-building stage so binary packaging does not fabricate artwork. The resulting source has null artwork until real PNG paths are supplied; publication rejects that incomplete metadata. Copy the product source and its relative files under the catalog source folder and add the record to products. Supply -Thumbnail and -Banner to copy approved artwork automatically.

A real APB 2.1.0 candidate was built from target/v3-release/release/verux_apb.exe and the drayebr/build-crypto driver matching the selected script. It is under target/packages/apb-launcher-2.1.0-r5. It has not been installed or published, and its artwork remains deliberately unset. Failed draft build directories were retained as diagnostics.

The new dedicated release key is stored privately under the current user's LocalAppData/Verux/ReleasePublisher directory. Its public companion is used by the launcher Build.ps1. Back up the private file securely; it is not copied into either repository or the Worker.

## Current implementation record — 2026-10-19

### Live rollout update

#### Explicit replacement — latest state

- Live catalog revision 3 advertises installer package 2.1.2, prepared under `target/packages/launcher-live-prepared-r3`. Its client executable remains the original APB 2.1.0 build; the installer revision adds migration support. The first 2.1.1 wrapper was superseded after command-binding and inherited-permission review, without changing published bytes.
- Use the newly rebuilt launcher. Signed optional `replacement_service` metadata enables REPLACE per product; the launcher contains no APB-specific service identity. Earlier launcher builds reject this new metadata and must be replaced with the current executable.
- REPLACE displays the existing service path and requires confirmation. Ordinary INSTALL/UPDATE continues to block foreign service ownership. The launcher captures whether the service is running before MSI stops it and passes that state with the explicit replacement flag.
- The package uses the original setup integrity/certificate/key provisioning flow. Validate checks payload hashes and the public certificate before migration. A protected ProgramData journal records the old service path, old-file hash and running state; no transport key is copied into it. Original files remain in place. Rollback restores the prior path/state, refuses a changed original file, and keeps the journal on failure. Commit removes the journal after managed-path/running-state verification.
- Migration diagnostics: `%PROGRAMDATA%/VeruxAPB/Logs/EchoDriver-migration.log`. Interrupted migrations require explicit elevated `Migrate-Driver.ps1 -Action Rollback` from the installed DriverPackage directory before retrying; inspect the journal and logs first. Do not delete the recovery record to bypass it.
- Launcher release build/format checks, PowerShell parsing, WiX compilation, native MSI identity inspection, signed catalog validation, and live package SHA-256/size verification passed. A read-only service query confirmed command binding; the development driver still runs from its original path. No migration/install/rollback execution or new automated tests were performed.
- Revision 3 expires **2026-10-20 13:23:19 UTC / 21:23:19 Manila**. Signing renewal is still not automated.

- The supplied logo was converted to the launcher ICO; the supplied 1280x720 APB artwork is used for its thumbnail and product banner. Product banners now accept 16:9 through 8:1, with identical publisher/client checks.
- The release launcher rebuilt successfully. Full APB catalog preparation exposed and removed an unused, undefined `mapping.push` statement in the publisher.
- R2 S3 credentials were loaded privately from the operator-provided local file into the publishing process only. No credential values were printed or committed.
- Signed catalog revision 1 and all referenced objects were published from `target/packages/launcher-live-prepared-r1b`.
- Worker deployment succeeded: `c265b1f4-8135-4b75-9892-e7c26de16b88`. Existing working-tree surface delivery changes were included; deployment used `--keep-vars`. TypeScript validation passed.
- The live `/launcher/v1/catalog` passed the launcher's native pinned-key verification (exit 0). Every downloaded image and APB MSI matched its signed SHA-256 and byte length. `/health` returned `ok`.
- This catalog expires **2026-10-20 12:55:24 UTC** (20:55:24 Manila). Publish a fresh higher revision before then. No periodic signing job has been configured.
- APB installation, driver start/update, reboot, repair, rollback, and uninstall remain unqualified on a test PC. Initial launcher distribution still uses the locally built executable until an official download location is selected.

The following notes record the earlier preparation state, before this live rollout:

First live installation attempt on the development PC stopped at MSI LaunchConditions with 1603: the existing EchoDriver ImagePath is `\??\C:\Users\Admin\Documents\source_repos\drayebr\build-crypto\EchoDriver.sys`, outside the package-owned Program Files path. This was an ownership rejection before installation actions, not a certificate failure. Do not remove that guard or silently take over the development service. A clean test PC or an explicitly authorized migration is required. The recovery journal remains available for retry. Download progress now emits percentage events to the UI while logging the download phase only once per transfer.

- Release launcher built at ../verux_launcher/target/release/verux_launcher.exe.
- Native signature validation accepted the prepared bootstrap catalog using the compiled public-key pin.
- Final APB candidate: target/packages/apb-launcher-2.1.0-r5/packages/verux.apb-2.1.0.msi. WiX compilation/validation and native MSI identity inspection passed.
- The candidate contains the actual APB 2.1.0 EXE and matching existing driver bytes/scripts. No driver installation ran.
- Cloudflare bucket verux-release-artifacts was created in the existing account. Its r2.dev public access was checked and is disabled.
- Worker TypeScript and deployment dry-run passed. The Worker was not deployed; its live launcher endpoint currently returns 404, which the launcher displays visibly.
- Product artwork is deliberately unset in the APB candidate source. No pretend catalog entries are published.
- R2 S3 publishing credentials still need private provisioning; no upload was attempted with substituted credentials or another transport.
- Back up the newly initialized private release key securely. Only its public companion is compiled into the launcher.
- Launcher updates require fresh signed catalog metadata. Reissue a higher revision before the configured expiry, even when package bytes do not change. Do not schedule a signing job without deciding where the private key will be held.

To collect several product releases without colliding filenames, use New-CatalogSource.ps1 with -Brand, -ProductSource (one or more product.source.json files), -LauncherSource when applicable, -OutputDirectory, and -Revision. It copies each product under its own ID namespace and rewrites local source paths. Missing real artwork blocks this step. Then use release.mjs prepare, validate, and publish on that collected source.

The selected local build compiler is WiX 4.0.6 at %LOCALAPPDATA%/Verux/BuildTools/wix-4.0.6/wix.cmd. It is not included in customer packages and changes no Windows security settings.
