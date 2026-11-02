# Dataset, training and inference algorithm delivery

Status: Dataset numerical generation and complete training fitting migrated in source; private artifacts and Worker deployed 2026-10-04. New release build and runtime qualification are recorded below. Native UI/capture/storage remain local. The inference migration is recorded below.

## Scope and user behavior

Keep recorded datasets and learned model weights on the user's computer. Move proprietary dataset-generation and training code into private, immutable server artifacts, delivered encrypted and compiled in memory. No frame, cursor history, sample, recording, or weight data is sent to the server to perform this migration. The server hosts the code; computation occurs on the user's computer.

Dataset and Train each load the required code when invoked. They must not require Start or retain code in the runner. Prepare asynchronously using percentage-only progress, with completion only after authorization, verification, compilation, and initialization succeed. Drop each task's executable runtime when it finishes, is cancelled, closes, expires, or loses authorization. A later invocation fetches anew. Existing model/dataset files remain local and retain their validated product/source/schema contract.

## Existing boundaries

- src/ml_ui.rs launches Dataset in a separate process using --ml-dataset and --ml-source.
- src/ml_mosaic.rs combines UI/cursor capture, task state, target-course generation, sample collection, and persistence.
- src/ml_aim.rs validates local dataset/model storage and calls training; it also contains basis, interpolation, and fitting helpers.
- src/ml_talisman.rs contains the fitting and personality-sequence algorithm as well as inference.

Keep window rendering, OS cursor capture, serial input, file ownership, and schema validation on the native host. Move pure course-generation/state transitions and training math behind bounded typed runtime contracts. Leaving all training math in native helpers and downloading only an entry-point stub does not fulfill the migration.

## Required contracts

Define separate product-scoped artifact identifiers and immutable versions for dataset and training. Preserve the signed manifest's nonce, recipient, session, product, ABI, digest, size, and expiry bindings. Extend the existing artifact allowlist explicitly; do not accept arbitrary algorithm names or R2 paths. Each contract defines maximum sample/trial counts, finite coordinate/time constraints, cancellation checkpoints, numerical compatibility, runtime fuel/memory limits, and maximum output bytes. The training budget must be measured independently of the tiny smoothing budget.

Do not give WebAssembly filesystem, network, device, or arbitrary memory imports. Use a bounded data-transfer ABI for local samples and output weights, validating every buffer range and returned length before copying. Preserve local validation before inputs and after output; server-delivered code is authenticated but must still be resource bounded.

## Dataset subprocess authorization

The parent app's existing session is authoritative. Do not perform another activation merely to authorize the dataset process; activation replaces the current session. Do not pass credentials in command-line arguments or files. Use an explicit, bounded parent-child channel to deliver authenticated preparation data and authorization/cancellation state. Bind the child/task identity and reject stale task generations. Failure to communicate stops visibly; a boolean CLI flag is not authorization.

The parent continues normal heartbeat/accounting. Child runtime access ends when the task lease expires, parent authorization is lost, or the parent exits. Dataset and training usage follow the existing connected-session policy unless the user explicitly changes that policy. Memory-only delivery does not prevent extraction by a customer-controlled operating system.

## Qualification and rollout

Build a numerical reference and compatibility inventory from the existing code before moving it. Validate owned complete courses, both coordinate sources, malformed/oversized local files, training cancellation, expired task leases, missing/tampered artifacts, process exit, and repeated invocation. Do not change native training behavior unrelated to the migration. Do not add automated tests or fixtures without user authorization. Keep a focused validation record and coordinated server/client rollout. No embedded formula fallback.


## Implemented task lifecycles

The shared `receive_algorithm` path verifies the signed protocol 2 envelope, exact algorithm/version/ABI, nonce, recipient, session, expiry, size and digest before releasing decrypted bytes. Every fetch remains license-authorized and rate limited. The numerical execution interfaces remain separate; a downloaded entry stub cannot dispatch a native fitting fallback.

Training: `src/forge_runtime.rs` fetches verux.apb.training on Train and compiles in a new Wasmtime engine/store without imports or caches. Native fitting and interpolation helpers were removed from the app's training path. The private artifact contains the regression, directional fitting, tracking normalization and learned residual construction. Learned output still passes host schema/numerical/identity validation. Input limit is 32 MiB, 32 datasets and one million samples; output limit is 16 MiB; runtime memory is capped at 192 MiB, with five billion fuel units and a 60-second execution deadline. A 100 ms epoch callback checks cancellation and the current session. Compilation remains bounded by the artifact size and is outside that execution deadline. Model persistence occurs in the owning UI after cancellation and session-generation checks, never directly from a late worker result.

Dataset: `src/mosaic_runtime.rs` verifies verux.apb.dataset in the parent app and sends only decrypted module bytes through newly created private anonymous pipes to its owned dataset process. No credentials or executable artifacts are passed in CLI arguments or written to disk. The child trusts this owned local channel; the environment marker only rejects accidental direct launch and is not authorization or an integrity attestation. Fresh pipes and the owned Child handle associate each preparation with its task. The child forbids imports and bounds memory at 4 MiB and fuel at 100,000 per numerical call. Course targets, jump interpolation, correction bands, post-acquisition motion, visibility/switch timing and approach radius are evaluated in the received module. Native host code retains rendering, input capture, sample validation, lifecycle and persistence.

The parent sends absolute Windows boot-tick lease deadlines, conservatively shortened by 250 ms, and renews from the existing authenticated session every 500 ms. The child cannot extend time by restarting a relative timer when a buffered grant arrives. Pipe EOF, cancellation, changed/expired parent session, or a zero grant stops the task. Every dataset event and numerical call checks the lease. The child acknowledges runtime initialization before preparation reaches 100%; the parent bounds readiness waiting at 20 seconds and kills/reaps a failed or abandoned child. Parent logout/authorization failure/exit closes the owned ML task. This is not cryptographic protection against an administrator controlling both processes.

## Deployed artifacts and validation

Worker: `3cae9278-413c-4beb-8a63-52ca354c9313`. Original smoothing and Songmaker licensing behavior are preserved; no D1 migration was needed.

- Dataset: 56,883 bytes; SHA-256 `d448b6cd05a11b8c77156946ec9332310a204ca35ec8b98941988eb6175ddf27`.
- Training: 180,179 bytes; SHA-256 `140fe619c8b88cda8756f940f7df7bbd585a7d4c94294d0afd2d233f51e3b8fd`.
- Explicit server allowlist maximums: smoothing/dataset 128 KiB, training 1 MiB. Signed algorithm response cap: 3 MiB. Request/body/license caps are otherwise unchanged.
- Both immutable artifacts were compiled with locked dependencies and uploaded to private R2 paths under their own algorithm identifiers. TypeScript check, clean Wrangler dry run, production deploy, and Rust check passed.
- Manual in-memory Node WebAssembly qualification: no module imports; 640 Dataset numerical comparisons had zero difference against the prior course formulas. Invalid operations were rejected. Training reproduced three owned stored models, comparing 124,953 numbers each, with maximum absolute difference 6.821210263296962e-12. Diagnostic inputs and outputs stayed local; no files or model weights were overwritten.

The user's prior Start/pause/restart/performance report is accepted. New Dataset/Train UI course completion and cancellation still require qualification of these newly migrated paths; the prior report does not imply testing code that did not yet exist. No automated tests, fixtures or test-only dependencies were added.


## Final native qualification and release

An owned manual diagnostic invoked the actual public `ml_aim::train` API through the app's native library, activating the locally protected saved license against the deployed Worker, verifying/decrypting the delivered artifact, and executing it in Wasmtime with the production resource and authorization checks. It matched all 124,953 compared numbers with maximum difference 6.821e-12. Debug-library end-to-end duration was 39.43 seconds, including network and compilation; this is not a release performance estimate. It did not save a model or overwrite a dataset, and closed its temporary license session afterward. The diagnostic can replace an existing app session; log in again if needed.

The initial standalone diagnostic did not link because Cargo's native resource/library paths were absent. The second attempt included incompatible ARM64 Windows import libraries and failed. The final attempt supplied the correct x64 native paths and passed. These were diagnostic-tool linking failures; no implementation fallback or validation check was weakened. Diagnostic source was provided to rustc through stdin, outside the repository; no automated test files or fixtures were added.

`cargo build --locked --release --bin verux_apb` passed in 7m 05s. The final executable is `target/release/verux_apb.exe`, 14,113,792 bytes. Rust check, TypeScript check, clean Wrangler dry run, immutable private uploads, production deployment, and focused whitespace checks passed. The interactive Dataset process/channel readiness, complete-course collection and user cancellation have not been exercised end-to-end in this pass. Node numerical checks are evidence for the received formulas, not a substitute for that UI qualification.


## Inference migration — 2026-10-04

`verux.apb.inference`, artifact version 2.0.0 / ABI 1, contains the full former flick-selection scoring, DMP basis and integration, feature construction, context-conditioned prediction and tracking sequence selection/interpolation. Native `ml_aim::Controller` is now an authenticated runtime adapter; native `ml_talisman` retains only the serializable weight schema and validation. No embedded inference implementation or fallback remains in the app. Other non-ML runner algorithms remain native.

Weights are read locally, validated, serialized once into the bounded module memory and never uploaded. Per-frame ABI calls are scalar coordinates/time with a status code and two scalar results; there is no JSON serialization or networking per prediction. Original algorithms and operation ordering are retained. Failed authorization, ABI, size, fuel, model or output checks stop visibly; failure remains latched until a new runtime is prepared.

Start preparation loads smoothing and inference on a background thread with percentage-only progress. It verifies the selected model ID, source, resolution and FPS before the prepared controller enters the runner. Runner cleanup drops inference and smoothing before waiting for worker shutdown; Pause exits that runner. Preview prepares asynchronously in the parent app and uses the same anonymous-pipe code/deadline protocol as Dataset, with readiness acknowledgement and a 20-second parent initialization bound. Preview checks authorization on every window event even when paused; parent EOF/cancellation/session expiry closes it. The environment marker is now VERUX_ML_CHANNEL and is not an authorization credential. The local channel is not protection against an administrator.

Inference limits: artifact 1 MiB, local encoded model 32 MiB, Wasmtime linear memory 128 MiB, one instance/memory/table, table limit 4096, initialization fuel 200 million and per-frame/reset fuel 100 million. Host imports and disk compilation caches are forbidden. Engine compilation is bounded by artifact size and cancellation is checked before/after compilation; there is no claim of instantaneous compilation cancellation. Model validation/host file reads retain their existing native allocation behavior. Remote code executed on a customer-controlled computer remains extractable.

Artifact: 141,964 bytes; SHA-256 `9936a25db208ac2450885a927c1c10d94f5a5a016c6eb4b9a998b52a885e8f57`. Uploaded to private immutable R2 path `verux.apb/inference/abi1/<digest>.wasm`. Worker `86555cb0-3b25-4459-86c4-3dd5bddc8727` is deployed. The Leonardo chat was concurrently editing the same server; this deployment used a reproducible source snapshot at `Verux Secrets\VeruxServerSecurity/inference-deploy-20261004`, excluding its unfinished firmware route/catalog/limiter without modifying that chat's source files. Existing signing secrets, licensing, artifact identifiers and database were preserved.

Validation so far: locked artifact build, Rust check/library build, TypeScript check and clean snapshot dry run passed. Inline local Node diagnostics covered 3,600 prediction frames across three existing owned models, resets and invalid-coordinate/time/axis rejection; no imports were present. Node maximum observed frame was 2.91 ms, not a native release benchmark. No automated tests, fixtures or test-only dependencies were added. Initial snapshot preparation failed because Python used the default Windows encoding on a concurrently edited UTF-8 source; using explicit UTF-8 corrected it. Its incomplete snapshot TypeScript check also exposed an unrelated draft firmware typing error; the final inference snapshot excludes that route and passes without weakening inference checks.


Final owned native qualification activated the DPAPI-protected saved license and invoked the public `ml_aim::Controller::new` API against production. Signed manifest validation, decryption, digest checks, no-import Wasmtime compilation, bounded initialization and per-frame live authorization/fuel checks all ran. Three owned models / 3,600 frames with resets matched the previous native numerical controller with maximum absolute difference `1.7763568394002505e-15`. Debug-library Wasmtime frame times: median 0.0084 ms, p95 0.6363 ms, maximum 1.5842 ms; end-to-end 19.24 seconds included three downloads/compilations. These are this machine's diagnostic timings, not a guaranteed production full-loop latency. Invalid coordinates were rejected. No datasets/models were changed or transmitted. The temporary session was deactivated on exit; an app session replaced by the diagnostic needs Login again. Temporary diagnostic executable/debug symbols were removed.

`cargo build --locked --release --bin verux_apb` passed in 4m 36s. Final focused whitespace checks passed. Newly changed interactive Start preparation and Preview pipe/readiness/cancellation need interactive confirmation; numerical/runtime diagnostics do not assert that UI qualification. The original user-reported lifecycle/performance acceptance is preserved.
