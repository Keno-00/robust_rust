# Proposed Verux BS minimum feature set

**Status:** proposal only. No feature trim, BS runtime changes, or process inspection has been performed.

| Design item | Proposal |
|---|---|
| User purpose | Show validated player information in the currently loaded single-player bot match. |
| UI | Keep the APB Dear ImGui look. Keep only General, Memory, and Visuals destinations; remove Color detection, Aim, and Trigger controls from the BS product. |
| Feature master | Memory master controls whether BS memory initialization and its dependent visuals are available. |
| Source | EchoDriver is the sole process-memory transport. Do not use APB offsets, direct process reads, or color detection. |
| Edition / entitlement | A BS-specific product identity and storage namespace, enrolled through the existing authoritative license server. Do not reuse APB identity or secrets. |
| Required signal | Identify the process/module and game build, then validate the BS-specific root and player/entity data before publishing any values. Discovery and runtime reads must all go through EchoDriver. |
| Dependent controls | Process selection/status, driver readiness, validated player/entity list, and basic player ESP. No aim or trigger action in the proposed minimum. |
| On loss | Stop reads and clear stale entities when the process, driver, build, root pointer, or validation fails; show a visible actionable error and diagnostic log. Never switch transport or engine implementation. |
| Profile migration | Start with a BS-scoped profile schema. Import no APB offsets or saved feature state. |
| Evidence confidence | Low until PID 16488 and the loaded match are inspected through EchoDriver and candidate structures are independently validated. |

The first discovery milestone, if this scope is accepted, is read-only: confirm EchoDriver access to PID 16488, identify the engine/module/build, and establish a validated player-list signal. Only then should the proposed player ESP be implemented.
