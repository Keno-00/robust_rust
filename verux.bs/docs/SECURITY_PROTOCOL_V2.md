# Verux 2.0 security protocol and release procedure

Source implementation and production server rollout date: 2026-10-03. Live Worker version: `ec22fadd-840c-42fc-bdc2-c75e558c200d`. Private R2 storage, signing key, and D1 migration 0003 are deployed. Owned positive activation and signed deactivation have been verified. Client publisher signing and Start/performance qualification remain separate gates. See [validation and limitations](SECURITY_HARDENING_PROGRESS.md) and [future feature rules](FUTURE_FEATURE_SECURITY.md).

## Trust and version matrix

| Component | Version / policy |
| --- | --- |
| APB executable | Cargo and Windows resource version 2.0.0; UI 2.0 |
| Authoritative server package | 2.0.0; enrolled-product minimums remain separate |
| APB requests | Protocol 2 only; no unsigned-response acceptance or automatic downgrade |
| Existing enrolled clients | Explicit protocol 1 compatibility on license endpoints; existing product minimums retained |
| Algorithm delivery | Protocol 2 only; APB smoothing 2.0.0, ABI 1 |
| Runtime | Wasmtime 49.0.2; ordinary validated WebAssembly, no native artifact deserialization |
| Delivery encryption | HPKE DHKEM(P-256, HKDF-SHA256), HKDF-SHA256, AES-256-GCM |
| Response signatures | Ed25519; public key IDs pinned at compile time |

The server stays in this repository. Every Verux product must adopt these controls verbatim, including response authentication, request bounds, accounting, limits, cleanup, and release gates. Only product constants, product-specific numerical contracts, and strictly necessary platform adaptations may differ. Document every necessary deviation. Do not copy the license authority into a product repository or create absent Payhip credentials. Existing Songmaker enrollment and its minimum version are retained; no Songmaker algorithm is invented or implicitly substituted.

## Signed responses

Every APB 2.0 license/delivery request contains `product_id`, `protocol_version: 2`, and a fresh canonical base64url 32-byte `request_nonce`. TLS certificate validation and redirect rejection remain enabled.

The response envelope is `{key_id, payload, signature}`. `payload` is base64url of the exact UTF-8 JSON bytes containing `{protocol_version, product_id, request_nonce, route, http_status, issued_at, expires_at, result}`. Ed25519 signs those bytes directly. The client verifies the signature before interpreting any result, then verifies the product, request nonce, route, HTTP status, protocol, and timestamp window. Issued/expiry timestamps are integer Unix seconds; maximum validity is 120 seconds, with 300 seconds of system-clock tolerance. Fresh unpredictable request nonces prevent responses from earlier requests satisfying a later request.

Signing uses exact transmitted bytes, avoiding cross-language JSON canonicalization. The server private key is never compiled into the app. `license-server/security/public-keys.json` contains public verification pins only. Owned payload buffers are zeroized where practical. Decoded result strings and runtime executable mappings are not guaranteed to be overwritten by the allocator or OS.

Responses before a valid signed context exists (edge rate rejection, malformed input, missing signing configuration, infrastructure failures) may be unsigned. APB 2.0 rejects them visibly; it never treats them as authorization. Established coordinator errors are converted to sanitized JSON before outer signing.

## Session accounting

License operations serialize through one Durable Object per product and canonical stored license hash. Normalize surrounding license-key whitespace before selecting the coordinator; preserve existing APB legacy hashes. Fetch and alarm settlement share the same concurrency gate. A D1 partial unique index additionally permits only one unrevoked session per product/license.

Heartbeat sequence must be exactly the prior sequence plus one. Elapsed usage is added in the same D1 batch as the session cursor update. Logout, replacement activation, and expiry settle the outstanding interval once, bounded by the last issued lease. Repeated settlement does not charge twice. Fractional Unix seconds avoid repeated short sessions receiving free time through rounding. D1 INTEGER affinity retains fractional values as REAL when required.

A missing logout charges the remaining authorized lease until expiry. An alarm settles expired sessions and rearms for the earliest live session; incoming requests cannot postpone an existing alarm. Alarms are recovery/accounting work, not precise real-time timers. They may run late, but settlement caps at expiry. New activation settles old state before granting another session. Migration 0003 charges/revokes duplicate legacy sessions and keeps the newest before creating the invariant index. Schedule a maintenance cutover so the old uncoordinated Worker cannot write during this migration.

Finite entitlements cap every new/renewed lease at whole remaining seconds. A remaining fraction below one second cannot issue a new lease. The client derives a conservative monotonic deadline from the request start time; UI delays or network time do not extend it. The runner and input sender stop on local expiry. Heartbeat failure clears authorization. Usage continues while an authenticated session remains open, including time paused in the UI; this preserves the existing connected-session policy.

Bearer tokens remain credentials. Installation proof-of-possession is not implemented in this revision; installation IDs/hashes are identifiers, not cryptographic client integrity attestation. An attacker controlling a customer's computer can use its session and patch its checks.

## Algorithm contract

`POST /v2/algorithms/prepare` accepts only an active authorized session, allowlisted product/algorithm/ABI, fresh nonce, and raw uncompressed P-256 ephemeral recipient public key. No frame, target, capture, coordinate, model input, or profile data is sent. The server checks the paid lease, product, revocation, service availability, artifact digest, and limits. Delivery shares the edge session ceiling (1200/minute per edge IP) and adds 30/minute per product/license. Cloudflare rate limits remain approximate and per location, rather than global accounting.

Private R2 object path: `verux.apb/smoothing/abi1/<sha256>.wasm`. Artifact maximum: 128 KiB for smoothing/dataset and 1 MiB for training/inference. The configured SHA-256 digest is checked before encryption. The bucket must have public access disabled, with no public custom domain.

The result contains `manifest` (the exact JSON text), `encapsulated_key`, and `ciphertext`. The outer signed response authenticates the manifest and encrypted payload. The manifest contains product, algorithm ID, immutable artifact version, ABI, SHA-256 digest, size, session hash, recipient key, request nonce, and session expiry. HPKE uses info `verux-algorithm-v2` and the exact manifest UTF-8 bytes as additional authenticated data. All binary values use canonical base64url without padding.

The client validates bindings, decrypts, hashes, checks current authorization, compiles in memory, forbids every host import, and binds the typed `axis(i32, f64, f64, f64) -> f64` export. Store limits: 4 MiB memory, one instance/memory/table, at most 1024 table elements, 100,000 fuel units for initialization and each pair of axis calls. Invalid numerical inputs, nonfinite/out-of-range outputs, traps, or missing artifacts stop visibly. There is no disk cache, persisted payload, alternate endpoint, embedded numerical substitute, or downloaded DLL.

The initial smoothing boundary covers clamp, linear, signed logarithm, scaled hyperbolic tangent, and quadratic Bezier. Dataset-generation math and training fitting are now additional on-demand private artifacts; see [their task contracts and validation](ML_ALGORITHM_MIGRATION.md). Existing native reference helpers are compiled only for existing tests, never used as a runtime fallback. Model inference now uses its own private server-hosted artifact, loaded before Start or Preview and executed locally in memory; other runner algorithms remain native. Model weights and recordings intentionally stay local. Further code migration needs separately documented contracts and qualification. No full-product secrecy claim is made.

A fresh preparation worker owns each load. Start is disabled while loading or while the previous runner is still shutting down. Settings cannot change during preparation. Cancellation disconnects the result and marks the worker cancelled; late results cannot install code. The runner owns its runtime and drops it before joining cleanup workers. Pause, stop, expiry, authorization loss, or exit releases it; a later Start fetches anew. Network/compilation work stays off the render thread. The progress bar exposes percentages only; it does not conceal traffic or make extraction impossible.

## Maintainer preparation and cutover

```powershell
# Repository root: build the immutable algorithm and update its public digest.
powershell -NoProfile -ExecutionPolicy Bypass -File license-server/scripts/publish-algorithm.ps1
cargo check --bin verux_apb
cargo build --locked --release --bin verux_apb

# license-server: validate source and local schema.
npm ci
npx tsc --noEmit
npm run db:migrate:local
npx wrangler deploy --dry-run
npm audit
```

Before production, validate an owned staging license through activation, Start, all curves, cancellation, pause/restart, short leases, concurrent/replayed heartbeats, replacement login, logout, wrong product, missing/tampered artifacts, poor connectivity, and server errors. Confirm actual Wasmtime and full-loop latency; Node WebAssembly microbenchmarks do not establish it. Do not probe production customers. No new automated tests/fixtures are added without user authorization.

After review and staging:

1. Back up D1 and verify restore permissions; record current Worker version, schema, limits, pins, and artifact digests. Preserve usage totals and enrolled-product settings.
2. Provision private R2 using `npx wrangler r2 bucket create verux-algorithm-artifacts`. Verify public access remains disabled. Reuse an existing owned bucket of this exact name rather than creating a public alternative.
3. Run `scripts/initialize-security.ps1 -Upload` from `license-server`. The generated PKCS8 key is protected using Windows CurrentUser DPAPI outside the repo at `Verux Secrets\VeruxServerSecurity/verux-20261003.pkcs8.dpapi`. Back up the signing key through an approved protected recovery mechanism; DPAPI data alone is not a portable backup. If the key is unavailable, recover or deliberately rotate it; never silently replace the public trust root.
4. Run `scripts/publish-algorithm.ps1 -Upload`. Confirm the object digest and Worker digest agree before deployment.
5. During coordinated maintenance, stop new activations on the existing Worker, wait out outstanding leases, apply `npm run db:migrate:remote`, then deploy the prepared Worker using `npm run deploy`. Resume service deliberately. Existing protocol 1 license behavior remains supported; expect existing sessions affected by the cutover to log in again.
6. Verify the owned staging/customer-authorized release flows against the deployed Worker, then publish the APB 2.0 client. The old unsigned Worker cannot authorize APB 2.0. Do not distribute the client ahead of its server.
7. Sign the release with the approved production publisher certificate using `scripts/sign-release.ps1 -CertificateThumbprint <approved-thumbprint> -SignToolPath <Windows-SDK-signtool.exe>`. The script requires SHA-256 signing, RFC3161 timestamping, Windows publisher-policy verification, and the expected certificate. Generate distribution hashes after signing. Local driver/test certificates are not approved production publisher identities.

Authenticode procedures follow [Microsoft SignTool documentation](https://learn.microsoft.com/windows/win32/seccrypto/signtool) and [DigiCert RFC3161 timestamping](https://knowledge.digicert.com/general-information/rfc3161-compliant-time-stamp-authority-server). An approved production signer and SignTool must be supplied; this change does not fabricate a publisher certificate. No updater is present, so update-signature verification requires its own future feature contract before an updater ships.

## Rotation, rollback, and remaining work

Add a new public key ID to client pins first, distribute clients supporting both keys, securely provision the new server private key, then change the server signing ID/key together. Retire the old key only after its supported clients have upgraded. A compromised private key requires emergency rotation and a supported-client update; an old pinned client cannot learn trusted revocation from an unauthenticated message.

Retire algorithms by disabling their server allowlist or replacing the required digest/version deliberately, with coordinated compatible client releases. Already delivered code cannot be recalled from an attacker. Do not delete artifacts still required by supported clients.

Roll back to a reviewed compatible Worker/client pair with the required schema and bindings. Preserve authoritative usage, revocation, product identity, and pinned trust. Migration 0003 must not be undone merely to admit duplicate sessions. There is no automatic offline or unsigned fallback.

Remaining gates: owned positive activation/Start and lifecycle validation, approved Authenticode signing, installation proof-of-possession, broader algorithm migration, full-loop Wasmtime performance measurement, backup/alert/retention operations, and any future verified updater. Generic string encryption, anti-debugging, or obfuscation is not claimed. Memory observation, Cheat Engine, and reverse engineering remain possible on customer-controlled machines; server-only execution is required for secrets that must never be delivered.
