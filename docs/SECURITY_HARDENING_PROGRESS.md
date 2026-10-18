# Security hardening progress

## 2026-10-07 expanded native naming and catalog deployment

The remaining selected native modules, feature types, fields and consumers now use Aurora, Comet, Lantern/Halo, Atlas, Orchid/Grove/Spark/Prism, Drift Guard/Tide/Arc/Velvet, Mosaic/Talisman/Mirage, Forge/Oracle, Canvas/Quill, Echo Courier/Passport and Lattice/Petal/Folio. Native paths and current references were updated together. Public protocol identities, saved model schemas, authoritative `license-server`, signing pins, existing secrets and protected key storage are preserved. Explicit aliases migrate both legacy plaintext and already-encrypted profiles. Existing local profile settings compared unchanged. Config replacement code, UI and every bundled asset remain identical.

Corrected Cloudflare authentication enabled publication of the key, D1 table and 132 encrypted rows. Encrypted-row digest: `2c7deacdaa787988e375e35d74bad2a6d94734efa76fc52194649b1b8b71e773`; deployed Worker: `66d2aa60-d31f-409e-93dc-9edda0a36170`. Production health and an owned live native signed/HPKE delivery plus actual UI-frame decoding of all 132 rows passed. The diagnostic session was signed-deactivated. Catalog validation wipes row temporaries immediately; frames decrypt only requested labels, then zeroize their owned strings on drop. Toolkit copies, displayed text, protocol data and administrators inspecting RAM remain outside this protection.

Rust all-target checks, TypeScript, dry run, final release and six offline identity captures passed. Of 36 existing library tests, 35 passed; the existing DXGI desktop test failed again in isolation with an empty buffer following a no-frame result versus its 30,000-byte expectation. No fallback, test weakening or unrelated capture repair was introduced. No new automated tests or fixtures were added. Release: 14,231,552 bytes, SHA-256 `04bc6ae389e125da8d64256bac86b2d6bd8cb01827168da097c9aba2164d572f`; 28 selected former distinctive strings were absent from ASCII/UTF-16 scans. Full scope and qualification: [encrypted display and profile implementation](ENCRYPTED_SURFACE.md).

## 2026-10-07 encrypted display and profile storage

Native identifiers use Unicorn, Pegasus and Beesmas. The 132 existing display labels are stored as opaque AES-GCM rows, delivered through the existing authenticated signed/HPKE server route, and opened only at UI rendering. The app profile file now uses a product-bound DPAPI vault with an explicit legacy import and atomic writes. Existing local settings were encrypted and verified unchanged, with protected recovery bytes retained outside the repo. Config replacement implementation, UI and bundled assets are identical to their pre-change snapshot.

Rust all-target checking, three existing config tests, six offline memory captures, TypeScript, Worker dry run, local D1 loading, manual HPKE/display round trip and isolated native profile/corruption qualification passed. Release SHA-256: `0777691d6b5db3f3a47819d1f368ade6400e29b9b9dc4f64757797b63c4ddf8e`; 14,257,664 bytes. Eight distinctive protected labels were absent from both ASCII and UTF-16 scans. Production publication and live UI qualification remain pending because Wrangler is authenticated to a different account from the configured Verux server. No production change was made. Full scope, limitations and maintainer commands: [encrypted surface](ENCRYPTED_SURFACE.md).

## 2026-10-07 native pointer encoding

Native APB offset/type metadata, cached module/camera/pawn/World/controller
addresses and roster identities now use shared XOR/rotation encoding. Build
material comes from Windows RNG; cached addresses use a separate process key.
Copied pointer-bearing fingerprints are masked as well. Initialization fails
explicitly rather than selecting an unencoded path. This is reversible
obfuscation, not code encryption or an authorization control. Existing signed
HPKE module delivery, licensing, UI and EchoDriver transport are preserved.
App/probe checks, optimized build and all six existing offline capture replays
passed. See [scope, limits and validation](NATIVE_POINTER_ENCODING.md).

Updated 2026-10-03. APB package version is **2.0.0** and its UI displays **2.0**. The server rollout completed on 2026-10-03; client distribution/signing and positive end-to-end qualification remain separate gates.

## 2.0 implemented source

- Per-product/license Durable Object serialization, including expiry alarms; D1 migration 0003 enforces a single unrevoked session and settles duplicate legacy records.
- Fractional usage settlement at logout, replacement, heartbeat, and expiry; idempotent settlement; normalized license identity; leases capped to remaining paid time; conservative client monotonic expiry.
- Ed25519 signed protocol 2 responses with compiled public pins and nonce/product/route/status/time binding. APB 2.0 rejects unsigned authorization. Existing enrolled protocol 1 license clients remain explicitly supported server-side.
- Session-gated private R2 delivery of the immutable smoothing WebAssembly module; HPKE P-256/HKDF-SHA256/AES-256-GCM encryption bound to its manifest and ephemeral recipient; per-license preparation limits.
- Per-run Wasmtime compilation in memory, no disk cache/host imports, bounded memory/fuel, typed numerical exports, finite/output checks, and zeroized owned plaintext buffers.
- Background Start preparation, settings/duplicate-Start exclusion, cancellation, percentage-only progress, and runtime release before worker cleanup on pause/stop/error. No per-frame internet requests. Native reference smoothing helpers are excluded from release execution.
- Maintainer key initialization/upload, immutable artifact build/upload, and mandatory-certificate release-signing scripts. A new server signing key was generated locally and DPAPI-protected outside the repository; no Payhip secrets were fabricated, inspected, or committed.
- Wrangler updated to 4.147.0, resolving the earlier CLI shutdown hang. Dependency audit reports no vulnerabilities.

See [the protocol, exact limits, lifecycle, release procedure, and remaining gates](SECURITY_PROTOCOL_V2.md). Future features follow [FUTURE_FEATURE_SECURITY.md](FUTURE_FEATURE_SECURITY.md). Other Verux products must adopt the same controls holistically, adapting only documented necessary product/platform differences.

## 2.0 validation

- `cargo check --bin verux_apb`: passed after runtime/UI/lease integration.
- `npx tsc --noEmit`: passed.
- `npm run db:migrate:local`: migration 0003 applied successfully; production was not touched.
- `npx wrangler deploy --dry-run`: passed and exited cleanly with all required bindings and artifact digest.
- `npm audit`: zero vulnerabilities, including development dependencies.
- Algorithm release build (`wasm32-unknown-unknown`): passed; 1958 bytes; SHA-256 `6708c8a3e46d614af05ddfe9ee46978bf21c99a93b4195eb48e872e06f2b6784`.
- Manual in-memory numerical diagnostics: 175 comparisons across five curves, signs, and smoothing values matched reference calculations; maximum absolute error 2.84e-14. Node WebAssembly compilation ~8.74 ms and axis-call average ~0.083 microseconds on this machine. These measurements do not establish Wasmtime or real-game performance.
- Manual in-memory SQL diagnostics used the actual production settlement module: APB logout charged 10.25 seconds, repeated logout left the total unchanged, SM expiry charged 10 seconds, and the unique index rejected a second active session. These are focused diagnostics, not full concurrent Worker/Payhip integration qualification.
- Manual response-signing diagnostic verified the genuine signature and rejected a tampered payload; cache policy was `no-store`. No private key or customer data was printed.
- `cargo build --locked --release --bin verux_apb`: final rebuild passed in 8m 11s after cleanup refinements. The executable is `target/release/verux_apb.exe`, 14,013,440 bytes; Windows product/file version is 2.0.0. Authenticode status is unsigned pending the approved production signer.

No new automated tests, test files, fixtures, or test-only dependencies were added. Significant unrelated work already present in the checkout was preserved. The root-level `npx tsc` command was accidentally started outside `license-server` and interrupted; the intended check was then run in `license-server` and passed. An intermediate Rust check caught a misplaced runtime binding during integration; it was corrected and the check passed. No validation failure was bypassed with an alternate implementation.

## Production server rollout — 2026-10-03

User explicitly authorized deployment and signed into the existing server-owner Cloudflare account. The initial authorization error was resolved by that login; no alternate server/account was created. Cloudflare required R2 activation, which the user completed in the dashboard. Wrangler is now pinned to the correct existing account in project configuration.

- Exported a pre-migration D1 backup outside the repo under `%LOCALAPPDATA%/VeruxServerSecurity/backups/20261003-192318/passport-db-before-v2.sql`. Backup contents were not included in source/docs.
- Confirmed the prior deployed APB/Songmaker product identities, minimum versions, and existing secret names; retained them.
- Uploaded `SERVER_SIGNING_KEY_PKCS8` using the existing protected local signing key. No secret value was printed by the upload script.
- Created `verux-algorithm-artifacts`; confirmed r2.dev public access is disabled and no custom domains are attached. Uploaded the 1958-byte immutable smoothing artifact matching the configured SHA-256.
- Deployed maintenance mode (`APP_ENABLED=false`), applied remote D1 migration 0003 successfully, then deployed normal service (`APP_ENABLED=true`). The initial preflight found zero active sessions; a later pre-migration check found one outstanding session. The migration preserves the newest session and ran behind the newly deployed coordinator; the planned full lease-drain wait was not performed. Record this sequencing deviation rather than claiming an idle database throughout cutover.
- Final Worker version: `c631ca22-c3bc-466c-9edb-4ce8ffa74327`; endpoint: `https://verux-license.verux-services.workers.dev`.
- Live health returned HTTP 200. Invalid random-session heartbeat and preparation requests returned signed HTTP 401 responses whose Ed25519 signatures verified against the client pins, with correct nonce/product/route/status binding. Both APB and Songmaker protocol 2 refusals were verified; Songmaker protocol 1 retained its expected JSON refusal. These checks did not use customer license keys or grant test sessions.

A positive licensed algorithm-delivery/Start flow, full runtime cleanup/performance qualification, and publisher signing still need their own evidence. A deployment and authenticated refusal checks do not establish those results.

## Pending release and security gates

Production migration/deployment, private bucket/artifact setup, and signing-secret upload are complete. Owned positive licensing/Start/pause/cancellation checks, actual Wasmtime/full-loop latency measurements, and approved production Authenticode signing remain pending. Only local driver/test certificates were found, and SignTool was not available on PATH; neither was silently used as a release identity. Backup/recovery, alerts/retention, installation proof-of-possession, broader algorithm migration, and verified updater behavior require subsequent qualified work. Other local algorithms and model weights have not been offloaded. Generic string encryption and invasive anti-debugging are not implemented.

Algorithms delivered for local execution remain inspectable and extractable. Encryption in transit, zeroization, and unload-on-pause do not prevent a customer-controlled OS from observing code while it runs, patching checks, or retaining a memory copy.

---

# Earlier first-pass record

Date: 2026-10-02. Scope: bounded first pass from [the implementation plan](SECURITY_IMPLEMENTATION_PLAN.md), leaving allowance for another running task.

## Implemented in source

- Shared server request-security module: bounded streaming reads, UTF-8/JSON-object validation, deadlines, strict token/version checks, and sanitized diagnostics.
- Independent edge-IP rate buckets before parsing; product/license and product/session HMAC buckets; deactivation limits; bounded Discord input.
- Bounded, timed Payhip verification with redirect rejection, documented enabled/key checks, and variant-only entitlement selection.
- Server configuration validation for heartbeat interval, session TTL, hashing-secret length, and supported version syntax.
- Rust client response-size/content-type limits, redirect rejection, required status fields, HTTP/schema consistency, activation token/interval checks, and entitlement arithmetic validation.
- Explicit APB product constant, stored installation identity length checks, safe-integer sequence ceiling, and shutdown failure logging.

No server secrets were invented, printed, or committed. No database migrations, production deployment, new automated tests, fixtures, or test-only dependencies were added. Existing unrelated working-tree edits were left in place.

## Validation

Executed from the repository root or `license-server` as appropriate:

```powershell
cargo check --bin verux_apb
cargo build --release --bin verux_apb
npx tsc --noEmit
npx wrangler deploy --dry-run
```

- Rust check: passed after final formatting and shutdown-log refinement.
- TypeScript: initially failed because Cloudflare's TextDecoder type requires `ignoreBOM`; corrected the constructor, then passed.
- Wrangler dry run: successfully bundled and listed all six rate-limit bindings, then printed `--dry-run: exiting now` but did not terminate. Both invocations were interrupted; disabling optional telemetry did not resolve the lingering process. Bundling succeeded, but clean CLI completion is unverified; do not count the dry run as a fully passing command.
- Release build: passed on the final source revision (`Finished release profile`).
- Manual request-module diagnostics, compiled in memory using the existing esbuild dependency: JSON object accepted; arrays/null rejected with 400; malformed JSON rejected with 400; wrong media type rejected with 415; oversized streamed/declared bodies rejected with 413; invalid UTF-8 rejected with 400; canonical identity accepted; invalid base64 trailing bits rejected; version suffix rejected.

No live licensed activation, actual edge rate-limit enforcement, or end-to-end UI workflow has been verified. No production customers were probed. Dry-run bundling does not establish deployment or runtime correctness.

## Deployment compatibility gate

Validate owned Payhip variants using the documented `data.variant_name`, `data.enabled`, and `data.license_key` fields before deployment. This source intentionally rejects missing/ambiguous fields instead of selecting a tier from customer data. Clients must send a canonical 32-byte base64url installation ID and numeric `major.minor.patch`; the current APB client does so. Confirm other enrolled clients comply before cutover. Deploy new limiter bindings with the Worker. Existing D1 records and product-specific secrets remain authoritative.

## Next implementation boundary

Correct usage accounting and concurrency together: serialize per-license operations, settle logout/replacement sessions, define disconnected-session lease charging, and guarantee one active session under simultaneous activation. Do not patch only logout and call accounting secure. Preserve product migrations and existing usage totals.

After that: signed/encrypted artifact delivery, isolated memory-only runtime ownership, unload-on-pause/stop, plain 0-100% preparation UI, performance-qualified algorithm migration, and signed release distribution. No anti-debugging or algorithm secrecy guarantee is provided by this pass.

## Login freeze follow-up

The user reported a freeze associated with an unreachable Worker error during login. Source inspection confirmed that login verification and UI heartbeat calls performed synchronous networking on the render thread. Heartbeat networking also held the shared session mutex throughout the request. These are confirmed blocking paths, although the user's exact runtime failure has not been reproduced through the UI.

Changed login submission to a background worker and nonblocking result polling, disabling repeated submission/input edits while a request is pending. UI heartbeat checks now use background work as well. Heartbeats release the session mutex during networking, permit only one in-flight request per session, and discard results for stopped/replaced sessions. Login success discards stale UI heartbeat results. Added a 15-second ureq overall network deadline alongside existing connection/read/write timeouts; system DNS resolution may exceed ureq's deadline, but no longer runs on the render thread. Transport failures log their category without credentials and show retry/support guidance. Shutdown network requests no longer hold the session mutex.

The configured default `/health` endpoint resolved and returned HTTP 200 with `{"status":"ok"}` from this machine. This does not prove licensed activation succeeds, nor that the user's earlier connectivity problem is resolved. No endpoint fallback, certificate bypass, or offline authorization was added.

Validation: `cargo check --bin verux_apb`, `cargo build --release --bin verux_apb`, and scoped `git diff --check` passed on the final revision. The rebuilt executable is `target/release/verux_apb.exe`. Interactive login-failure reproduction remains unverified. No automated tests were added. Other runner cleanup/join operations remain outside this narrowly scoped login fix.


## Production Payhip connection fix — 2026-10-03

The deployed Worker returned `upstream_unavailable` during login. Credential-free tail diagnostics isolated the failure to the upstream fetch call, before reading a response. Replacing fetch redirect mode `error` with `manual`, with explicit rejection of all HTTP 3xx responses, restored verification. No redirect is followed and the Payhip secret remains restricted to the fixed HTTPS API origin. The exact runtime exception from the earlier mode was not recorded; it must not be logged verbatim because it may contain the license URL.

Retained bounded upstream timeout/body handling and added sanitized stage/status/transport categories. Deployment `747461d6-052d-4241-8544-4620a4066666` completed successfully after `npx tsc --noEmit` passed. An invalid diagnostic key received HTTP 401. The user's locally DPAPI-protected saved license then received HTTP 200 / `valid` through protocol 2; its Ed25519 signature, nonce, product, route, HTTP status, and expiry were verified with the committed public pin. The temporary activation was closed immediately with a verified signed `deactivated` response. Credentials stayed in process memory and were not printed or written to diagnostic files. This activation may replace an existing app session, so the user should log in again. UI Start and performance qualification remain outstanding. No new automated tests were added.


## User qualification and expanded scope — 2026-10-04

The user reports that Start, pause, restart, unloading, and performance were good in their manual use. Record this as user-reported functional qualification, not an instrumented performance benchmark or validation of all failure/race cases. Do not ask them to repeat the same ordinary happy-path check as a prerequisite for continuing migration.

Model weights and recorded dataset contents stay local. Dataset-generation and training algorithms join the server-hosted, encrypted, memory-only delivery scope. Their loads occur on Dataset and Train actions independently of Start. The separate dataset process needs an explicitly authorized task lifecycle; it must not silently reactivate the license and replace the main app's session, accept a command-line authorization boolean, or write executable artifacts/tokens to disk. See ML_ALGORITHM_MIGRATION.md.

The Leonardo setup product is named verux.leonardo. Reuse the existing Verux banner and UI style. Routine controls have terse names and no explanatory prose. Error dialogs still require actionable messages. The implementation remains pending; naming and this scope record do not constitute a completed setup executable.

EchoDriver was inspected read-only at the installed service path in the sibling drayebr repository. Authenticode verification on this machine reports Valid with subject WDKTestCert Admin. This is a locally trusted development identity, not evidence of Microsoft production signing or customer compatibility. No driver configuration, certificate, Secure Boot, or code-integrity setting was changed.


## Dataset/Train implementation and rollout — 2026-10-04

Dataset generation math and complete training fitting now use separate encrypted, signed, private server artifacts loaded on their own actions, without Start. Recorded datasets and model weights remain local. Dataset gets code and conservative renewal deadlines through private anonymous parent-child pipes; it does not activate another session. Training is isolated in a no-import Wasmtime store with bounded memory/fuel/deadline, cancellation and live-session checks. Cancelled/old-session training results are discarded before model persistence. Host UI/capture/storage/validation and model inference remain native. No embedded training or dataset-formula fallback was added.

Worker `3cae9278-413c-4beb-8a63-52ca354c9313` and both immutable private artifacts are deployed. TypeScript, Rust check, clean dry run and release build passed. Manual numerical checks covered 640 Dataset cases and three existing training models; an owned live signed/encrypted Wasmtime training invocation also reproduced the reference model without saving output. Full evidence, budgets, diagnostic failures and the remaining new Dataset UI qualification are in ML_ALGORITHM_MIGRATION.md.

The user requested a separate project/chat for verux.leonardo deliberation. Created C:/Users/Admin/Documents/source_repos/verux.leonardo with DESIGN.md and inherited AGENTS.md, and a separate chat. Its design discussion has completed its initial turn; its setup executable remains unimplemented. No Leonardo firmware was changed during this migration.

Driver packaging need not contain source: distribute the required compiled driver and installation artifacts, preserving the existing files/signatures. Keep C/C++ source, project files, debug symbols and private signing keys out of the customer bundle. The user reports customers accept the development-mode prerequisites; no installation mode or Windows security setting was changed in this task.


## Inference rollout — 2026-10-04

Complete ML inference numerical logic now joins smoothing, Dataset and Train as a private server-hosted encrypted artifact, with local in-memory execution and local weights. Start and Preview prepare it asynchronously. No per-frame server request, local inference fallback or new automated tests were introduced. The Worker inference release is `86555cb0-3b25-4459-86c4-3dd5bddc8727`; the concurrent Leonardo draft was excluded from its deployment snapshot. See ML_ALGORITHM_MIGRATION.md for the complete runtime contract and validation. Other non-ML runner logic remains native.


## Personalized firmware delivery deployed — 2026-10-04

User authorized publication and deployment. The initial two-identity catalog deployment (`1105fb3e-8f3d-457f-b5d1-36047ea6822c`) was too narrow for other users and was immediately superseded. Current Worker: `ec22fadd-840c-42fc-bdc2-c75e558c200d`. The server accepts the selected USB mouse VID/PID as integers 1–65535 and personalizes a verified private template; no per-mouse catalog or compilation is required. This does not assert Windows device merging or physical compatibility with every mouse driver.

Template version `1.0.0-candidate.1`, SHA-256 `f9d35d821fa7396516cad9035e10c11d470f3e104f4ac4d56021d771aa21286e`, 78,860 HEX bytes, private R2 path `verux.apb/firmware/leonardo/template/1.0.0-candidate.1/<digest>.hex`. Configuration: LEONARDO_FIRMWARE_TEMPLATE with descriptor_offset 242. The source descriptor's entire 18-byte sequence is checked before only bytes 250–253 (VID/PID) are changed. Input HEX checksums, EOF, uniqueness and complete 28,672-byte application coverage are mandatory. No bootloader bytes are allowed. Generated HEX checksums/digest are recalculated and bound into the signed/encrypted session/recipient/nonce/identity manifest. Other application bytes stay identical. Replacing the compiled template requires reviewing its descriptor offset, expected bytes, digest, version and compatibility; never search/patch arbitrary matching machine-code bytes.

TypeScript and Wrangler dry run passed. Local inline diagnostics checked IDs 0001:0001, FFFF:FFFF, 1234:5678, 046D:C08B and 4C4A:9002, verifying decrypted image identity and every other application byte unchanged. Live owned signed activation/delivery/decryption/digest/HEX/descriptor checks passed for 1234:5678 and both detected mice. Invalid identity bounds received a verified signed 400. Temporary sessions were signed-deactivated; credentials were read from existing DPAPI storage, never logged or written plaintext. These activations may replace an existing app session, so Login may be needed again. No serial ports were opened and no physical flash occurred. Interactive Install/physical readback/re-enumeration qualification remains pending. Existing setup executable understands the same manifest and needs no rebuild for this correction.

Historical per-identity immutable objects remain private but are unused by the current delivery configuration. Server scope, signing secrets and algorithm digests were preserved. No new automated tests, fixtures or test-only dependencies were added. One TypeScript invocation ran from the repository root and found no local compiler; rerunning in license-server used the pinned compiler and passed without installing a dependency.
