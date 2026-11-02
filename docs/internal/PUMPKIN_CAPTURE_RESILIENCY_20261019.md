# Pumpkin capture resiliency — 2026-10-19

Source-internal; do not package or embed.

The user authorized withholding a changed sample and revalidating on the next survey, with a bounded stop for repeated instability. This corrects the logged `Pumpkin prefab or asset changed during capture.` worker termination at 12:55:10 Asia/Manila.

Purpose/category: keep Pumpkin ESP and radar acquisition reliable through confirmed prefab/asset replacement. Existing Pumpkin masters, Full/Memory eligibility, memory source, entitlement, and World requirements remain unchanged. Required evidence is a successful memory read proving that the prefab type/signature, asset pointer, or asset signature differs across capture. No alternative acquisition implementation, unchecked identity, remembered position, or substitute asset is used. No profile migration or new customer UI text is required.

`JadeSurvey` records affected addresses and withholds them for the entire survey, including samples accepted earlier in that same pass. Their owning level is scheduled for fresh discovery next survey even if its actor-array membership is unchanged or it is outside the usual round-robin selection. Fresh discovery obtains a new reference and repeats every existing identity, asset-name, coordinate, level, World, and prop-association check. Coordinates are validated after the final identity checks, so demonstrably mixed-instance samples are discarded while invalid coordinates in a stable sample remain fatal.

The limit is five unstable captures per address, counted at most once per survey. Different objects have independent budgets. A stable/ineligible disposition clears a pending budget only after the survey completes; an early stable probe cannot erase a later mismatch from the same pass. Departure from the surveyed level caches and World changes also discard obsolete retry state. The fifth mismatch propagates an actionable error directly rather than passing through the existing actor-removal error classification. The existing worker reporter logs it and stops processing visibly.

Every discarded capture logs its address, phase and attempt count; recovery is logged once. The worker's existing 100 ms survey interval remains in charge of scheduling, without a hot retry loop. Driver/authentication failures and read errors for still-current members retain their existing fatal path. Existing successful-membership evidence for an object removed during a failed read is preserved. Unrelated name-table, prop, level, World, and invalid-data errors are not broadly reclassified.

Evidence confidence: the terminal message and its source branch are confirmed; the log cannot identify which of its identity comparisons changed or prove whether collection, despawn, or address reuse caused it. Gameplay acceptance remains separate from compilation/build checks. No automated tests, fixtures, or dependencies are added or run under the repository instructions.

Validation commands:

```powershell
cargo check --locked --quiet
cargo build --locked --release --bin verux_apb --no-default-features --features memory --target-dir target/v3-release
```

This is a Full client change; no server or catalog deployment is required.

Validation: `cargo check --locked --quiet` passed. The optimized Full release build passed in 7m48s and was copied to `target/editions/color-memory/release/verux_apb.exe`; source/copy SHA-256 match: `624FF22554E3660529F43D206125D2C90441C3814EED2D3C54460A3780D25502` (42,862,592 bytes). `git diff --check` passed. No live gameplay reproduction or recovery confirmation is claimed.
