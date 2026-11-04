# Pumpkins seasonal decommission — November 4, 2026

The user requested a reversible decommission flag, no Pumpkin delivery, hidden
UI references, and safely disabled behavior until a future season. The earlier
F8-only phase is complete; the user subsequently authorized this Verux UI work.
Notice-board work remains deferred. This document is source-internal.

## Eligibility decision

- Purpose: suspend seasonal acquisition/markers while retaining the implementation.
- Category: existing Visuals and its radar sub-option; no new category or label.
- Feature master: existing saved `jade` and `comet_jade` preferences, subordinate
  to Memory and the source-controlled commissioning flag.
- Source/edition: existing Memory capability and existing Full entitlement;
  licensing, supported-build checks and EchoDriver remain unchanged.
- Required signal: none while decommissioned; no acquisition should start.
- Dependents: visual markers, their appearance/copy-from entry, radar-object
  toggle, radar object acquisition, and SB Protect risk evaluation.
- Parent/World loss: existing rules remain; commissioning unavailability strips
  both Pumpkin consumers before preparation and runtime requirements are derived.
- Migration: none; retain saved booleans, visual-style slots and serialized keys.
- Evidence: code-path review and Full/Color compilation; no live UI or gameplay
  acceptance is claimed for this change.

## Implementation

`config::PUMPKINS_DECOMMISSIONED = true` is the single reversible source flag.
`ProfileConfig::eligibility().pumpkins` requires Memory capability and that flag
to be false. The flag is not a saved setting and profiles cannot override it.

`effective_config()` clears `jade` and `comet_jade` when unavailable. The stored
profile remains untouched. A previously enabled Pumpkin-only profile therefore
cannot request the memory/overlay pipeline solely for that feature. An enabled
independent player radar continues normally, with its Pumpkin option stripped.

The Visuals checkbox, radar sub-option, appearance selection and copy-from list
all use this capability. If the selected appearance was Pumpkins, the existing
selection logic moves to an eligible visual or hides the appearance controls
when none is enabled. No decommission message, placeholder, notice or new text
was added to the UI. Existing authenticated catalog IDs remain compatible.

The runner already obtains an effective config before deriving requirements and
starting object workers. With both requests false, it cannot start a Pumpkin
worker or feed its visual/radar markers. Projectile workers remain independent.
`ObjectWorkers::new` also rejects a direct request for the decommissioned source
before allocating a worker, with a diagnostic log and an actionable generic
error. It does not silently select another source or acquisition implementation.

## Reactivation

When the user authorizes the feature's return, set
`PUMPKINS_DECOMMISSIONED = false`, validate the seasonal implementation against
the then-current supported APB build/event, and compile the relevant editions.
The same eligibility path restores the existing controls and permits saved
preferences to become effective again. No profile rewrite, style-index change,
implementation reconstruction or new catalog ID is needed. Publication still
requires the operator's explicit go-ahead.

## Validation and handover

Both compilation checks passed from the main checkout, with output under
`target/`:

```powershell
cargo check --locked --offline --bin verux_apb
cargo check --locked --offline --bin verux_apb --no-default-features
git diff --check
```

Full completed in 43.60 seconds. Color completed in 1m55s and retained its existing
unused `encode_fingerprint` warning. The diff check passed. Code-path review
covered the two visual controls, the appearance/copy-from lists and selection
reconciliation, SB Protect, profile serialization, effective requirements,
preparation and the runner's single object-worker constructor call.

No automated tests were added or run, and no test-only files/dependencies were
added. Live UI/gameplay validation was not performed. No release executable or
package was produced, and nothing was deployed, published or added to the notice
board. Existing untracked `.wrangler/` and `captures/` were untouched. No files
were intentionally created outside the main checkout. The implementation and
these records are committed together; the installed/published release is unchanged.
