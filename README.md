# robust_rust

Current native naming, encrypted display delivery and profile storage: [implementation and release status](docs/ENCRYPTED_SURFACE.md). Older settings names below refer to the legacy import format.

Parallel Rust version of the Robust/Verux APB tool.

This repo is intentionally separate from the Python project. The sibling repo at
`../robust` is the behavior source of truth. The Python repo is the experimental
and personal codebase; this Rust repo is for the future performance-oriented
version with cleaner boundaries.

## Porting contract

- Preserve every user-facing behavior from `../robust`.
- Keep the UI absolutely faithful to the Python DearPyGui interface, using Rust
  Dear ImGui wrappers rather than inventing a new layout or control model.
- Keep config names, defaults, activation behavior, smoothing semantics,
  modifier behavior, target filtering, labels, and visible workflow compatible
  with the Python version unless a deliberate compatibility note is added here.
- Rust internals may differ where Python and Rust need different architecture.
  Backend structure, threading, ownership, and hot-loop implementation do not
  need to mirror Python line for line.
- Arduino is the required and only supported input backend in the Rust version.
  Do not implement or preserve ViGEm, GHub, MouseEvent, or other mouse backends
  except as legacy config migration aliases if needed.

## First goals

- Keep a faithful Dear ImGui-style UI.
- Preserve the user-facing feel of activation modes and aim modifiers.
- Keep config concepts understandable for users coming from the Python version.
- Build the core runtime around explicit modules instead of one large loop.
- Replace Python's selectable mouse methods with a single Arduino transport.

## Planned modules

- `canvas`: screen capture backends
- `detection`: color thresholding and target extraction
- `activation`: hold, toggle, timed, on-aim, target conditions
- `modifiers`: ramp, decay, proximity charge, future effects
- `smoothing`: movement math
- `quill`: Arduino serial transport only
- `ui`: Dear ImGui front end
- `config`: profile loading and saving

## Tooling note

The active APB detector now deliberately differs from the Python detector: fixed
BGR red thresholds, horizontal binary-mask reduction, per-name grouping, and
horizontal-only movement replace the configurable HSV/morphology path. Existing
profile files remain readable; legacy detection controls are inactive. See
[APB detector behavior and build instructions](docs/APB_DETECTOR.md).

Optional curve-matched predictive braking adds one checkbox and a persisted opt-in
profile value. Its future-landing estimate and movement limits are
documented in [Predictive braking](docs/PREDICTIVE_BRAKING.md).

Rust tooling was not available on this machine when the scaffold was created.
Install Rust with `rustup`, then run:

```powershell
cargo check
```

## Approved UI and memory integration (2026-09-30)

The approved layout replaces Basic Settings/Targetting/Misc with Aimbot,
Triggerbot, conditional Computer Vision, conditional Memory, and General.
Feature source radios and activation keybinds live in their feature tabs.
Source tabs follow selection, including when a feature is disabled. Existing
Dear ImGui controls and styling are preserved; explanation text, hover
explanations and routine status panels are removed. Action failures remain
visible as error dialogs and diagnostic log entries.

Player projections use the existing visuals renderer. New profile settings
are additive: `lanterns`, `lantern_key`, `lantern_key_name`,
`lantern_filter`, and four role colors. Older profiles retain their feature
source choices and default to projections off. Memory aim accepts only mission
opponents. Memory Y correction uses candidate live collision bounds to enter 25% inside
the body after starting outside it. Visible mesh scale and native live acceptance
remain unverified. See [native memory integration](docs/MEMORY_COORDINATES.md).

Build validation: `cargo check --bin verux_apb` and
`cargo build --release --bin verux_apb`. No new automated tests are included.

The October 18 core update targets APB SHA-256
`35AEF01734AAB20B4B0C622ADDF5706F9E75CAAA98D5B84A0EA14D9DF72EE453`.
Build the updated client with the hash gate enabled:
`cargo build --locked --release --bin verux_apb --target-dir target/v3-release`.
Offset changes and feature limitations: [October 18 core update](docs/APB_CORE_UPDATE_20261018.md).

Pumpkin/projectile markers and sustained-recency aim eligibility are described
in [object feature release notes](docs/APB_OBJECT_FEATURES_20261018.md). The
object controls require their separate encrypted interface catalog release.

For an explicitly requested APB build compatibility trial only, build with
`cargo build --locked --release --bin verux_apb --features apb-hash-test --target-dir target/v3-release`.
This test feature bypasses only the APB.exe SHA-256 match and logs a `TEST BUILD`
warning with the actual and expected hashes. Builds without the feature enforce
the hash gate. Runtime memory validation remains active during the trial.

### October 1 compatibility update

Humanize is a dedicated tab after Aimbot. Smoothing (including its curve and
value), Predictive Braking, Ramp, and Sticky move there with their existing
controls and profile keys. Ramp also retains its existing target-loss decay
behavior. The ML Aim dataset, training, saved-model management, model preview and runtime blend,
and the intentionally revised stateless smoothing curves, are documented in
[ML Aim and named movement curves](docs/ML_AIM.md).

Visuals is available again. Memory keeps process/service controls; ESP controls
move to Visuals. Aim Visuals and ESP retain independent switches/keybinds.
FOV Box, Aim Target Box and Aim Line have separate switches/colors. Aim Line
Origin offers Center/Top, and ESP Style offers Boxes/Dots (Boxes by default).
Display Distance labels verified character separation in meters using the
user's 100 world units per meter calibration.

Aimbot Target Priority offers Closest Aim and Closest Distance. Closest Aim
now uses screen distance to the target box center, including Computer Vision;
movement semantics remain unchanged. Closest Distance requires Memory.
The legacy projection profile keys remain readable, with the additional
settings documented in [memory integration](docs/MEMORY_COORDINATES.md).

Block now inhibits input only, leaving acquisition and visuals running.
Full camera table audits run in the roster worker; relocating the same camera
instance between slots no longer starts a 500 ms confirmation pause. The
optimized executable is `target/release/verux_apb.exe`; startup logs identify
memory pipeline revision `2026-10-01`.

The vehicle-exit update monitors bounded roster mutations every 10 ms and
refreshes the complete roster immediately on a change. On-foot ownership
uses the canonical collision component's two verified owner links instead of
waiting for registered component membership to repopulate.

Triggerbot's Also Activate Aim appears only for a Hold/Timed aimbot and a Hold
triggerbot. When enabled, holding the trigger key also acts as holding the
aim key. Hold releases when both keys are released; Timed retains its existing
0.5-second release period. On Aim and Block still apply. Both features must be
enabled for the link to act; the saved `trigger_activates_aim` setting defaults
off and is inactive for unsupported mode combinations.
