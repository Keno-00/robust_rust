# Cursor, hit events and vehicle confirmation — November 4, 2026

Follow-up to [fresh core discovery](APB_CORE_SCAN_20261104.md), using the same
APB PID 1452 / creation FILETIME 134382696964582094 and executable SHA-256
`35AEF01734AAB20B4B0C622ADDF5706F9E75CAAA98D5B84A0EA14D9DF72EE453`.
All process-memory reads used encrypted EchoDriver v3 through `Session.read`.

## Cursor and hit events

The operator pressed F8 to start the focused monitor, then labelled white/green
cursor states and tried hit events. The session ended normally with F11 at
21:01:57 Manila time. Cursor root `APB.exe + 0x3BD3340` reached `0x8E9D1600`;
the integer field remained `+0x134`.

- Twelve green labels read **48**.
- Seven white labels read **130**.
- The first white label read **48**. This discrepancy remains unexplained and
  is preserved in the raw log. Subsequent repeated white/green reversals matched
  the existing mappings; do not describe every label as matching.
- No red cursor was directly observed. The user explicitly instructed retaining
  red **99** when green remained 48 and said they likely would not test red.
  This is accepted compatibility behavior, not fresh empirical proof of red.

The cursor-bound timer `+0x14C` and RGBA `+0x150` produced two red rises
`(1,0,0,1)` with sound and two white rises `(1,1,1,1)` without sound. The operator
confirmed: “all is working as correctly”. The predicate remains a finite,
stable-binding sample and positive timer increase greater than 0.05; no pixel
source was used. These four observations do not establish exhaustive hit counts
or rapid-fire coverage. A red hitmarker is separate from red cursor state.

The initial diagnostic launch stopped visibly because it selected the old asset
name `[minecraft] hitsound.wav`. The repository's existing profile migration maps
that filename to `minecraft_hit.wav`; the diagnostic was corrected to that exact
asset. The failed launch log is retained. No alternate sound or reader was used.

## Vehicle entry and exit

The user selected vehicle entry/exit as the next check. The first attempt recorded
on-foot state for the driver-labelled snapshot and stopped with a nonzero exit.
The user said they were entering/exiting and pressing F8 during those changes;
the precise second-F8 settled state was uncertain. A second attempt rejected
changing HUD table entries before accepting a stage. Both failures are retained.

The helper now uses the existing diagnostic overlay to show the current step.
It requires a fresh F8 for each settled state. `ValidationRuntime` still rejects
changing bindings and roster snapshots; each rejected attempt is saved and shown
before another manual F8 is accepted. Driver errors and unsupported layouts stop
the run. The exact-launch profile and supported-build gate remain mandatory for
the explicitly selected `--profile` mode. The historical discovery mode is not
used as a fallback.

The third run accepted all three states without a rejected attempt:

| Time, Manila | Operator stage | Resolved state | Controller possession |
|---|---|---|---|
| 21:08:22 | On foot | `on_foot` | Pawn `0xA600C100` |
| 21:08:28 | Driver, settled | `vehicle_possession` | Vehicle `0x1718EA900` |
| 21:08:33 | Exited, settled | `on_foot` | Pawn `0xA600C100` |

All three retained the same local pawn fingerprint, World `0x94F4510` and
controller `0x7CA7C080`. The rooted HUD, reciprocal ownership, local collision
pose, roster membership and timestamp guards passed through the same reader.
The overlay closed after completion. Passengers, death/respawn, district travel
and full APB restart remain separate, unverified lifecycle cases at this point.

The user then chose to stop this round because of limited time and accepted
leaving district-change and full-restart live checks pending. Source inspection
supports reasonable confidence for the unchanged executable: native camera
resolution rechecks the module-rooted table/slot and typed fingerprint, World
changes request roster refresh, and new source connections obtain a fresh module
base and process identity. This is a code-based inference, not live transition
evidence. It does not establish seamless reconnection after APB exits. No new
offset discovery is indicated by this session; unexpected transition behavior
or a changed executable should trigger focused diagnostics.

## Changes, validation and handover

No product offsets or mappings changed. The red constant remains 99, with a
source comment recording the user's compatibility decision. No product UI,
feature eligibility, catalog or release changed.

The focused sound diagnostic gained opt-in cursor labels and F8 startup,
timestamped raw samples and the corrected asset path. The vehicle diagnostic
gained an explicit profile mode reusing the existing current-core validator and
overlay. Python syntax/import/help checks and `git diff --check` passed. Both
diagnostics were exercised live. No automated tests, fixtures or dependencies
were added; no product build was needed for the comment-only product change.

Complete event logs, failed launch records, rejected vehicle runs and successful
vehicle snapshots are in
[the evidence archive](evidence/apb-core-confirmation-20261104-1452.zip).
The [manifest](evidence/apb-core-confirmation-20261104-1452.json) records hashes,
CRC validation and a compact findings summary. Original working copies remain
under `captures/`, alongside preexisting untracked captures. `.wrangler/` is
untouched. Nothing was deployed or published, and no files were intentionally
created outside the main checkout. EchoDriver remains running.
