# robust_rust

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

- `capture`: screen capture backends
- `detection`: color thresholding and target extraction
- `activation`: hold, toggle, timed, on-aim, target conditions
- `modifiers`: ramp, decay, proximity charge, future effects
- `smoothing`: movement math
- `input`: Arduino serial transport only
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
