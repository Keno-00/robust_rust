# SB Protect and unified visuals — 2026-10-19

Source-internal implementation notes. Do not embed or package this document.

## Approved scope and eligibility

Purpose: user-defined configuration warnings and consistent visual appearance. General owns the SB Protect master (default on). Visuals groups activations into Aim, ESP, and Radar, followed by one enabled-only Configure selector. Settings disappear when their feature is off. Copy appearance is a one-time copy, not a shared mutable style.

Source/edition: existing `ProfileConfig::eligibility()` remains authoritative. Target box/lines require Aim; player/object ESP and radar require Memory and the Full edition. FOV remains an independent screen visual. Color FOV represents capture bounds; Memory Aim FOV represents a crosshair-centered circular selection radius in client pixels. Its radius is configured in Aimbot and is identical in selection and rendering. Selection measures the projected body center and retains the existing player priority, filters, visibility gate, and source. No source substitution is introduced.

Required signals and losses: existing verified body bounds, role, camera, object state, and position signals are reused. Existing focus/World/district/revocation handling remains in charge of runtime teardown and withholding frames. Visual settings neither grant access nor start memory consumers without eligibility. Rainbow/breathing run from elapsed runtime time. Alpha rendering uses bounded GDI raster layers and a per-pixel-alpha layered window. Allocation/drawing/presentation errors reach the existing runtime error reporter and log; no alternate renderer is selected.

## Approved SB rules

`src/sb_protect.rs` is the only risk evaluator. Inactive/unavailable children do not contribute. Aimbot and Triggerbot take the maximum active child risk. Unrated controls remain neutral white; the colors are user-defined recommendations, not measured detection probabilities.

| Setting | White | Yellow | Red |
|---|---|---|---|
| Aim / Trigger source | Color | — | Memory |
| Aim activation | Toggle; any mode bound to LMB/RMB | Other Hold/Timed | — |
| Trigger activation | Double Crouch; Hold bound to RMB | Other Hold; Toggle with any key | — |
| Vertical tracking | No Y | Default; Closest Point with all four points | Fixed Head/Neck/Torso/Feet; Closest Point with 0–3 points |
| Aim player filter | Opponents / Mission Players | All Players | — |
| Visible Only (Aim only) | On | — | Off |
| Smoothing | On | — | Off |
| Smoothing value | >3 | 2–3 inclusive | <2 |
| Smoothing type | Bezier | Hyperbolic | Linear / Logarithmic |
| Predictive Braking | On | — | Off |
| Ramp | On | Off | — |
| Ramp time | >300 ms | 250–300 ms inclusive | <250 ms |
| Ramp idle reset | >=50 ms | <50 ms | — |
| Ramp Apply / easing | All | — | — |
| Sticky (called Proximity during discussion) | Off | On | — |
| Sticky distance / easing | All | — | — |
| Sticky build time | >300 ms | 250–300 ms inclusive | <250 ms |
| Sticky effect | — | Strength / Smoothing Value | — |
| ML | On | — | Off |
| ML influence | >=90% | >=50%, <90% | <50% |
| ML management controls | All | — | — |
| Trigger delay | >125 ms | >80 ms, <=125 ms | <=80 ms |
| Trigger red/green / activates Aim | On and Off | — | — |
| Visual activations / General Memory | Off | — | On |
| Hit sound | On and Off | — | — |

Trigger has no separate visibility checkbox or player filter; no controls or risk checks were invented for them. Empty Closest Point remains invalid at Start, independently of its red recommendation.

After the first rendered baseline, an edit (including a binding or profile change) from white to red opens one modal: `This setting increases risk.` with `OK`. It acknowledges the applied setting; it does not undo it. Red-to-red changes and steady frames do not repeat it. SB off suppresses coloring and warnings. Enabling SB establishes a fresh baseline. Both the transition identifiers and acknowledgement are logged.

## Appearance and migration

Each of seven consumers (FOV, target, line, players, pumpkins, projectiles, radar) stores its own appearance: Solid/Rainbow/Breathing, color, opacity, width, black/custom outline, outline width/color, optional fill, fill opacity, animation speed, saturation/brightness, and breathing minimum opacity. Applicable geometry includes full/corner/circle boxes, corner length, marker size, line origin/endpoint, ESP distance limit/labels, and radar range/placement/scale. Source-dependent FOV geometry is fixed to match the actual acquisition region.

Player/radar relation palettes and distance-label flags are independent per consumer. Object visuals can retain their verified state colors. Copy appearance copies palette/animation/stroke/fill only; it preserves geometry, distance, filters, placement, range, activation, and source. Palette inheritance is disabled when copying into visuals without relation/state colors.

Profiles persist `sb_protect`, `memory_aim_fov`, `radar_range`, and `visual_styles_v1`. Profiles without the appearance key migrate legacy RGB, role palettes, box/dot choice, and distance visibility. Object limits retain the former 80 m range; radar retains 100 m. A missing Memory FOV migrates to half the smaller capture dimension (clamped to 1–4000 px); the circular boundary is an intentional approved source-behavior change. Existing `vertical_tracking=current` remains the saved enum value, displayed as Default. Malformed appearance is logged and visibly blocks editing/saving/Start until the profile is restored; it is never rendered using a substitute.

## Authenticated catalog and release

New clients request product-scoped `surface.visuals`. The older surface/objects/eligibility/hitsound delivery paths and digests remain supported unchanged. Full uses 191 rows; Color uses 189 (excluding Full-only sound IDs). Preparation uses each product's existing DPAPI catalog/key vault and encrypts with the existing product AAD. No missing key is invented.

Preparation (local only):

```powershell
pwsh -NoProfile -File license-server/scripts/prepare-visuals-surface.ps1
pwsh -NoProfile -File license-server/scripts/prepare-visuals-surface.ps1 -Product verux.apb.cb
```

Release requires uploading both prepared encrypted catalog SQL files to the authoritative D1 database and deploying the worker with the two new digest settings before distributing the new client. Existing clients continue requesting their existing catalogs. Preparation does not upload or deploy.

Separate release commands, from `license-server` (not executed during implementation):

```powershell
npx wrangler d1 execute LICENSE_DB --remote --file security/apb-surface-visuals-catalog.sql
npx wrangler d1 execute LICENSE_DB --remote --file security/apb-cb-surface-visuals-catalog.sql
npx wrangler deploy
```

## Validation commands

```powershell
cargo check --quiet
cargo check --quiet --no-default-features
cargo build --release --bin verux_apb
cd license-server
npx tsc --noEmit
```

No automated tests, fixtures, or dependencies were added. Build/typecheck results are separate from live UI and in-game acceptance, which remains required for presentation, timing, and performance on the target hardware.

Implementation validation: the Full check initially passed. A later Full check was blocked by six deleted/renamed `assets/wav/[minecraft] ...` assets still referenced by the existing hit-sound module; those concurrent audio edits were left untouched. The final Color check and server TypeScript check pass. Color reports the existing unused `encode_fingerprint` warning. No final Full executable was built, no server publication was performed, and no live UI/in-game acceptance is claimed.

## Authorized build and publication follow-up

The user then confirmed replacing the WAV library and explicitly requested build/deployment. `build.rs` now generates the embedded list from all supplied WAVs, checks their RIFF/WAVE headers, and rejects a missing library/default. The current folder contains 227 WAVs (28,420,406 bytes; PCM and Microsoft ADPCM). Fresh profiles explicitly select `minecraft_hit.wav`; existing profiles retain their selected name and fail visibly if that file was removed, requiring a new selection in General. Build invalidation tracks source/assets and Cargo inputs so pointer-encoding generation remains tied to source changes. Color continues compiling without the sound payload. Full `cargo check --quiet` and server `npx tsc --noEmit` pass after the update. No automated tests were added or run.

Both optimized releases passed. Color finished via `scripts/build-editions.ps1 -Edition Both` in 9m13s; the pending fresh Full dependency rebuild was intentionally stopped to reuse the existing Full cache. Full then passed `cargo build --locked --release --bin verux_apb --no-default-features --features memory --target-dir target/v3-release` in 3m35s and was copied to the edition release path with matching SHA-256.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `target/editions/color-memory/release/verux_apb.exe` | 42,858,496 | `B0C1F27B2574D768DBF2E2A3953488CDC6FCB5036F299DEE2C11F6247CD7A634` |
| `target/editions/color/release/verux_apb.exe` | 14,165,504 | `1F3B3D43AED993AA3125A53DC447BCC0AFB3CB66B2A91CEF5C2824196E9CC898` |

Both new catalogs were uploaded to the authoritative D1 database and read back with exact expected digests and row counts (Full 191, Color 189). Existing production catalogs matched their configured digests before release. Worker bundle dry run passed; `npx wrangler deploy` published version `8d67dd4e-c92d-4a56-b5d8-9898a399cb58` at `https://verux-license.verux-services.workers.dev`. The post-deployment `/health` read returned `{"status":"ok"}`. Runtime login, gameplay, and individual WAV playback have not been live-verified by this release step.
