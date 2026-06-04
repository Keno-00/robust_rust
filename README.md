# robust_rust

Parallel Rust version of the Robust/Verux APB tool.

This repo is intentionally separate from the Python project. The Python repo is
the experimental and personal codebase; this Rust repo is for the future
performance-oriented version with cleaner boundaries.

## First goals

- Keep a faithful Dear ImGui-style UI.
- Preserve the user-facing feel of activation modes and aim modifiers.
- Keep config concepts understandable for users coming from the Python version.
- Build the core runtime around explicit modules instead of one large loop.

## Planned modules

- `capture`: screen capture backends
- `detection`: color thresholding and target extraction
- `activation`: hold, toggle, timed, on-aim, target conditions
- `modifiers`: ramp, decay, proximity charge, future effects
- `smoothing`: movement math
- `input`: mouse, Arduino, ViGEm-style backends
- `ui`: Dear ImGui front end
- `config`: profile loading and saving

## Tooling note

Rust tooling was not available on this machine when the scaffold was created.
Install Rust with `rustup`, then run:

```powershell
cargo check
```
