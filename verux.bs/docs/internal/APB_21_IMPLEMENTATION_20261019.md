# APB 2.1 eligibility and editions — internal implementation ledger

## Checkpoints

Version-only commit aa26bfb was followed by full source checkpoint f9d1e27 before these changes. The checkpoint includes source, documentation, packaging and supplied WAV assets. Raw captures and local Wrangler cache remain local and uncommitted; no private commerce or display keys are embedded.

## Implemented scope

- One shared eligibility contract derives effective configuration for preparation/runtime and UI visibility. Memory Features off disables all memory consumers without switching source or bypassing memory hash validation. Aim and trigger children require their parent. Independent screen visuals remain available.
- Full default build includes Color+Memory; Color no-default build excludes actual driver transport, offsets and scans. Explicit unavailable adapters preserve shared interfaces without an alternate reader. Product IDs, profile/ledger/ML storage and signed resource identities are separate.
- Existing verux.apb licenses stay Full. Color listing verux.apb.cb uses a separate server-only Payhip secret, key, product hashes, storage prefixes and request identities. No existing enrolled product is renamed.
- ML lists/recordings/training/preview obey the selected source, clear incompatible active selection and cancel work when its parent becomes unavailable. Separate Color WASM modules enforce Color product and source identity; verified Full modules were not weakened or replaced.
- Aim > Targeting retains player priority separately from Vertical tracking. Modes: Current, No Y, Head, Neck, Torso, Feet, Closest Point with selectable subset. New anchors use approximate screen-height bounds (0%,18%,45%,100%), not real bones. No Y suppresses learned Y as well. Closest Point with no enabled anchors is an explicit Start error. New anchors are memory-only; Color retains Current/No Y. Unsupported saved approximate modes combined with Color fail visibly until explicitly changed; no silent mode replacement.
- docs/internal/DOWNSTREAM_DESIGN.md and AGENTS.md govern future categorization and user deliberation. Source-internal documents are not installer resources or embedded UI help.

## New authenticated display catalog

143-row surface.eligibility is separate from the legacy132 and objects134 catalogs. Full catalog digest 0bdeb9e0a251187d037287df6013430cecbba9b984e20d217c2d2fa1c939ce5c. Color digest 93526c2844a8d1d5ccc586bb1db62c06c78654872c0eccc35fcbb266c2dba853, separate key and product AAD. Shared wire artifact version remains2.0.0/ABI1; application version is2.1.0. Both SQLcatalog uploads succeeded, private secrets were provisioned through stdin, and Worker c87522c3-736d-482e-b55e-b39ac6bea735 deployed successfully. No customer credential is stored in this ledger.

## Validation and limitations

Full and Color native release typechecks passed. First Full typecheck exposed the new profile parser using a closure before declaration; corrected by moving the existing closure before parsing, then reran successfully. Color check reports one nonfatal unused fingerprint helper warning. Server TypeScript check and deployment preview passed. Three separate Color WASM release builds and private digest-versioned uploads passed. Full and Color native release builds succeeded; no automated tests were added or run.

The initial generic Color artifact copy included Full ML payloads. Inspection identified their strict Full product schema; they were superseded by separate Color builds before deployment. Reproducible enrollment now copies only shared smoothing/firmware; publish-color-algorithms.ps1 compiles and publishes the three Color ML modules.

User live acceptance of new controls, approximate point placement and Color purchase activation is pending. No updater, weapon labels, remote-look detector, hit/death audio or audio playback-state detector was implemented. Captured-name leads and exact evidence are in APB_NAME_DISCUSSION_20261018.md. New sound assets are checkpointed only; a reliable trigger and desired notification scope must be agreed before integration.


Final review caught a Trigger-to-Aim control shown without eligible Aim, and an unsupported vertical mode silently normalized. Both were corrected before rebuilding: shared validation runs before preparation and launch, and the trigger child hides without Aim. Full/Color typechecks then passed again. The earlier release build was deliberately interrupted so it could not publish pre-review code.


Full native release build succeeded in 4m 14s. Artifact target/editions/color-memory/release/verux_apb.exe SHA-256 29725E132F813ADBFAAFB4AADC2C9773574260732692E66E3A904B5245BF64B2. Color native release build then succeeded in 3m 58s. Artifact target/editions/color/release/verux_apb.exe SHA-256 F542E29B243B64C9B89F8407C36AE0CC98C30F7B48BAB5528143E4E2A91A8A7D. These outputs are not code-signed or auto-updater builds; that scope remains deferred.


Use the explicit edition artifact paths for launches. The shared target/v3-release directory was used only as build staging and its last executable is Color; do not use that generic staging path to launch the Full edition. No new native client was launched in this turn. Windows Full artifact file/product metadata is2.1.0. Automated tests and live purchase/game checks were not run.
