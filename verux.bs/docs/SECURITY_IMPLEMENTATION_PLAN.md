# Security implementation plan

Status: phased plan with partial 2.0 source implementation. Updated 2026-10-03.

Implementation status and remaining work: [hardening progress](SECURITY_HARDENING_PROGRESS.md). Request hardening, per-license coordination/accounting, signed responses, encrypted smoothing delivery, memory-only runtime ownership, and percentage-only preparation are implemented in source. See [the exact protocol and pending rollout gates](SECURITY_PROTOCOL_V2.md). The phases below are not all complete.

Companion: [future feature rules](FUTURE_FEATURE_SECURITY.md).

## Objectives and limits

Keep licensing authoritative on the existing Verux server. Deliver selected proprietary algorithms only after authorization, compile them in memory, execute locally without network calls in the processing loop, and unload them on pause or stop. Startup shows only a progress bar and percentage from 0 to 100; implementation stages are not displayed.

Assume an attacker can control the client OS, inspect memory, patch the executable, capture delivered payloads, and impersonate client requests. Memory-only delivery and encryption raise extraction cost; they cannot guarantee algorithm secrecy. LLVM compilation and WebAssembly sandboxing do not provide secrecy. Server-only execution remains the option for algorithms that must never reach a client.

Do not introduce debugger concealment, security-tool interference, or custom DLL/manual-mapping loaders. Use a maintained runtime with documented APIs. No silent local implementation, cached authorization, or alternate endpoint may replace a failed selected implementation.

## Phase 1: correct server authority before algorithm delivery

Location: `license-server/src/index.ts`, focused new server modules, reviewed D1 migrations, and `src/passport.rs`.

1. Strictly validate bounded JSON bodies, explicit product identities, token formats, bounded client versions, and positive finite safe-integer sequences. Determine entitlement from explicit trusted Payhip variation fields instead of searching arbitrary response strings. Preserve enrolled-product behavior through documented mappings.
2. Apply abuse controls before expensive body parsing and upstream calls. Add independent network and per-license/session buckets; installation IDs and token prefixes must not be the only limits. Cover activation, heartbeat, deactivation, and future delivery endpoints. Define upstream timeout, concurrency, request-size, and cost ceilings. Document Cloudflare limits as approximate per-location controls.
3. Serialize state-changing operations per product/license using a documented server-owned coordinator, such as a Durable Object. Migrate active licensing state deliberately; do not silently switch authorities. Recheck state inside the serialized operation. Preserve D1 usage and audit records, define crash recovery, and do not return success until authoritative writes succeed.
4. Settle elapsed usage on heartbeat, logout, and replacement activation. Define reservation/lease accounting so disconnecting before the first heartbeat cannot produce unlimited free use. Define rounding, stale-session settlement, and crash semantics explicitly. Enforce one active session even with concurrent activation.
5. Bind sessions to product, installation key identity, protocol version, and expiry. Installation proof-of-possession can reduce bearer-token-only theft; it is not proof of an unmodified client and a compromised user account can still use its key.
6. Client accepts success only on the expected HTTP status with a complete valid schema. Check timestamps, product, session, and version relationships. Fail visibly on malformed, expired, or contradictory responses. Log sanitized failure codes; never log tokens, keys, or decrypted payloads.

Exit evidence: manual concurrent activation/heartbeat checks; replay and stale-session rejection; repeated short sessions charge usage; malformed requests fail safely; product isolation remains intact. Use a dedicated staging database and licenses. Do not probe production customers.

## Phase 2: authenticated delivery contract

Keep the license authority in `license-server`; do not duplicate it in another product repository. Add an authorized delivery endpoint, for example `POST /v1/algorithms/prepare`, plus product-scoped private artifact storage such as R2. Artifact publishing is a separate maintainer workflow, never a client permission.

The client submits its active session, selected algorithm IDs, ABI/runtime version, fresh nonce, and an ephemeral encryption public key. The server independently checks entitlement and the allowlisted algorithm selection. No caller-supplied URL, source code, storage key, or secret selects what is executed.

Return a signed manifest containing product, algorithm IDs and versions, ABI, payload digest and size, issue/expiry times, request nonce, session binding, and recipient-key binding. Sign a precisely specified canonical byte representation with an asymmetric signing key held only on the server. Pin verification public keys in the client and document overlapping key rotation and emergency revocation. Signing authenticates the payload; TLS remains mandatory.

For encrypted delivery, select a reviewed standard/library implementation such as HPKE rather than inventing an encryption protocol. Bind ciphertext to the manifest. The client holds ephemeral private keys only for this load. Encryption prevents passive disclosure but cannot prevent extraction by the recipient. Bound compressed and decompressed sizes; avoid compression initially unless justified.

Publish immutable, versioned artifacts. Reject unknown, retired, cross-product, expired, altered, wrong-recipient, or incompatible artifacts. Never deserialize untrusted precompiled native runtime artifacts; start with validated WebAssembly input bytes. Authorization checks apply to every fetch. Do not expose a public artifact bucket.

Exit evidence: document the wire schema, canonical signing bytes, key ownership, version matrix, and manual rejection cases. Verify authorized delivery and cancellation without printing secrets.

## Phase 3: in-memory execution lifecycle

Proposed client boundaries: `algorithm_protocol`, `algorithm_runtime`, and `algorithm_session`. UI and runner receive typed state and function interfaces, not raw executable bytes.

Evaluate Wasmtime with WebAssembly compiled from received bytes in RAM. Disable disk compilation caches. Expose only typed numerical inputs/outputs, with no filesystem, network, process, or arbitrary host-memory imports. Bound memory and execution; validate finite numbers, output ranges, runtime traps, and ABI compatibility. Sandboxing protects the host from payload behavior; it does not hide the algorithm.

Lifecycle:

- `Idle -> Authorizing -> Receiving -> Verifying -> Compiling -> Initializing -> Ready -> Running`.
- Any load error enters `Failed`; cancellation enters `Stopping` then `Idle`.
- Pause, stop, logout, lease expiry, and shutdown enter `Stopping`: stop output, reject new calls, wait for in-flight calls, destroy instances and state, release all compiled-module references, and clear owned plaintext/key buffers using appropriate zeroization primitives.
- A subsequent start creates a fresh generation and fetches again. Late responses from old generations cannot install code. Selecting settings requiring another algorithm must stop and reload explicitly.

Use one disposable runtime worker process per run if needed to make code lifetime and fault isolation easier to verify. Use bounded IPC and restrict the worker's authority; authenticate the worker's executable through the release distribution chain. Measure IPC cost before adopting it. Process exit is not guaranteed erasure of pagefile, crash dumps, or previously captured memory.

Do not retain loaded code through pause for convenience. Audit reference ownership and runtime caches; dropping an instance alone may leave compiled code alive. Document every sensitive buffer and its destruction point. No promise of complete OS-wide forensic erasure.

Heartbeats stay outside the real-time computation loop. A runtime trap or authorization loss stops the affected functionality with an actionable error and diagnostic entry. There is no embedded substitute formula if remote delivery fails.

## Phase 4: progress UI and performance

Show the existing Start workflow with a plain bar and `0%` through `100%`. No stage labels, algorithm names, cryptographic terminology, tooltips, endpoint details, or explanatory status text. The bar is a user experience decision, not a security boundary.

Progress tracks completed preparation milestones and byte transfer where a trustworthy total exists. Reserve the final percentage for successful initialization; show 100% only when ready. Do not fabricate completion or expose a runnable state before preparation succeeds. Run preparation off the UI thread, bound timeouts, support cancellation, and prevent duplicate Start operations.

Failures still require a short actionable dialog, such as connection failure with retry guidance, and a sanitized diagnostic code. Private maintenance logs may record stage, duration, artifact version/digest, and correlation ID. Never include payload contents or session credentials.

Before selecting a runtime, capture the current native execution baseline. Compare startup duration and end-to-end loop median/p95/p99 latency, jitter, CPU, and memory using the same representative inputs and settings. Include pause/start cycles, poor connectivity, cancellation, and runtime failure. Define acceptable budgets from the baseline before implementation; do not equate the existing 100 ms stale-frame cutoff with available overhead. Loading delays are accepted by the user; no specific duration or zero overhead is promised.

First migration scope: one small pure calculation boundary used solely to establish numerical compatibility and overhead. Select which proprietary algorithms to migrate only after that evidence; no broad simultaneous rewrite.

## Phase 5: release integrity and operations

Sign release executables and updates; verify signatures before installing updates. Protect signing keys outside source control and ordinary build logs. Release optimized binaries without public debugging symbols; keep private symbols for diagnostics. Review shipped logs, error strings, paths, assets, and embedded secrets.

Consider selective string/code obfuscation only after correctness and performance gates. Treat it as extraction friction. Do not add invasive anti-debugging or claim memory monitoring is impossible. A generic formula interpreter, encrypted constants, and JIT code can all be analyzed.

Keep server Payhip, hashing, payload-signing, and storage credentials separate and server-only. Apply least privilege, dependency review, backup/restore procedures, sanitized audit retention, abuse alerts, and deployment rollback documentation. Signed old artifacts still need server retirement rules. A rollback restores a reviewed compatible version and preserves authoritative usage; it is not an automatic fallback.

## Delivery order and validation commands

Deliver focused changes in this order: licensing correctness; delivery protocol/storage; isolated runtime lifecycle; progress UI; performance-qualified algorithm migration; release/operations hardening. Each change records behavior, compatibility, validation, and unresolved risks.

Existing commands, run from their indicated directories when implementation changes:

```powershell
# Repository root
cargo check --bin verux_apb
cargo build --release --bin verux_apb

# license-server, after installing declared dependencies
npx tsc --noEmit
npx wrangler deploy --dry-run
npm run db:migrate:local
```

Remote migrations and deployment require a reviewed staging result and deliberate release scheduling. Do not add automated tests, fixtures, or test-only dependencies without explicit user request. Manual checks and existing relevant tests may supply evidence. Report failed validation directly.

## References

- [Wasmtime module compilation and artifact safety](https://docs.wasmtime.dev/api/wasmtime/struct.Module.html)
- [Cloudflare rate-limit semantics](https://developers.cloudflare.com/workers/runtime-apis/bindings/rate-limit/)
- [Client-side enforcement limitations](https://cwe.mitre.org/data/definitions/602.html)
- [OWASP REST security](https://cheatsheetseries.owasp.org/cheatsheets/REST_Security_Cheat_Sheet.html)
- [Windows release signing](https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool)
