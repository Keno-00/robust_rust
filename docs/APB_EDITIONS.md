# Verux APB edition builds

Both editions are built from this repository and keep version 2.1.0. They are separate artifacts, not a preference stored in a profile.

| Edition | Cargo features | Compiled capability |
| --- | --- | --- |
| Color | `--no-default-features` | Screen capture, color acquisition and shared client services |
| Color + Memory | `--no-default-features --features memory` | Color plus the hash-gated EchoDriver memory implementation |

Existing plain Cargo builds retain the Color + Memory capability through the default `memory` feature. The explicit `apb-hash-test` diagnostic feature implies `memory`; never use it for a release.

## Build commands

Run `powershell -File scripts/build-editions.ps1` to build both editions, or pass `-Edition Color` / `-Edition ColorMemory`. Outputs are isolated under `target/editions/color/release/` and `target/editions/color-memory/release/`. Each contains the one-file `verux_apb.exe` distribution. The script stops visibly on a failed build and prints the resulting SHA-256. It does not upload, enroll products or publish releases.

For a development typecheck, use `cargo check --locked --bin verux_apb --no-default-features` and `cargo check --locked --bin verux_apb --no-default-features --features memory`.

## Compile boundary

`src/lib.rs` excludes the real driver reader, driver service startup, cursor memory reader, memory target correction, APB hash/offset implementation, roster traversal, visibility reader and object scanners from a Color build using `#[cfg(feature = "memory")]`. `src/color_only.rs` contains only interface data shapes and explicit unavailable-operation errors so shared UI/rendering code can compile. It has no driver transport or scan implementation. Memory frame acquisition always returns an error; reaching memory-only targeting without acquisition is an invariant violation and stops visibly. Changing saved booleans cannot recover excluded code.

`edition::memory_available()` reports compiled capability only. It is not evidence of license entitlement. Existing license validation remains required in both editions. Full retains product `verux.apb`, its `VeruxAPB` directories, existing ledger AAD and existing fingerprint derivation. Color uses product `verux.apb.cb`, `VeruxAPBCB` directories, separate ledger AAD, product-scoped fingerprints and profile/ML envelope identities. Color payload requests and manifest checks use the `verux.apb.cb.*` resource namespace. Full resources remain unchanged. No existing customer state is imported across editions.

Separate server enrollment, encrypted resource catalogs, download artifacts and edition-scoped entitlements must be configured and audited before distributing distinct licensed editions. These client changes do not publish products or store commerce secrets in the executable.

This prevents a Color artifact from containing a dormant memory reader. It does not make a separately obtained full artifact unpatchable, and client-side edition checks cannot replace server authorization.

## Color ML artifacts

Dataset, training and inference WASM crates expose a `color-only` build feature. Training/inference retain strict schema validation against product `verux-apb-cb` and accept only Color recordings/models; the dataset entry point rejects Memory courses. Native Color validation enforces the same source and product constraints. Full builds retain their original `verux-apb` schema and both source types.

Run `powershell -File license-server/scripts/publish-color-algorithms.ps1 -Upload` to build isolated Color WASM outputs, upload versioned private `verux.apb.cb` artifacts and update only the three `APB_CB_*_DIGEST` settings after successful uploads. Omitting `-Upload` builds and reports hashes without changing release digests. Deploy the Worker separately after reviewing the release. Copying Full ML WASM into Color storage is incompatible with Color model envelopes.
