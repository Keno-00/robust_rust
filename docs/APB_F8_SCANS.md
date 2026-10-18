# APB F8-paced diagnostic collection

These diagnostics use separate elevated terminals. Each records the APB.exe
process identity (PID, creation time, executable path, SHA-256) and keeps its
own output directory and terminal log. F8 starts a requested capture only when
APB is foreground. There is no countdown for changing game state. Wait for
`CAPTURE COMPLETE` before moving to the next state. A failed scan stops with a
visible error and preserves completed phase files for inspection. None of the
diagnostics enables targeting or sends a `shoot` command.

All APB **memory bytes** in these diagnostics are obtained by `Session.read`,
which sends EchoDriver's read-process IOCTL through `\\.\EchoDevice`. There is
no `ReadProcessMemory` call. Windows APIs still identify the process and window,
`VirtualQueryEx` enumerates writable memory regions without reading their
contents, and Pillow captures client pixels. Thus "all reads through EchoDriver" applies to
APB process-memory contents, not OS metadata or screenshots.

Run one terminal at a time. Choose a **new** output path for every APB launch;
do not combine captures from different process identities. The scripts expect
the verified APB.exe build and EchoDriver read access from an elevated shell.
The camera tool also needs the installed Python NumPy and Pillow packages.

For discovery on a new executable, explicitly bypass the diagnostic hash gate
in that elevated PowerShell session before running the capture commands:

```powershell
$env:APB_SCAN_ALLOW_UNVERIFIED_BUILD = '1'
```

The scanner prints a warning with both the actual and verified hashes; capture
identity records retain the actual hash and mark unverified-build discovery.
The standalone discovery report records `build_verified: false` as well.
This override does not validate old offsets or change the product's hash gate.
Do not combine old-build captures with new-build captures. After validating the
new build and updating the verified hash, remove the temporary override:

```powershell
Remove-Item Env:APB_SCAN_ALLOW_UNVERIFIED_BUILD
```

The visibility discovery workflow now uses explicit F8-confirmed scene captures,
repeated reversals, a movement control and separate rechecks. Its sampling timer
starts after confirmation. See [APB_VISIBILITY_TRACE.md](APB_VISIBILITY_TRACE.md)
for the PC-stationary/laptop-controlled protocol and candidate/live commands.

## 1. Cursor state and pointer offset

The saved F8 finder is `scripts/apb_cursor_unknown.py`, launched by
`scripts/run_apb_cursor_unknown.ps1`. It captures white, red, white, red, and
green at your pace, narrowing changing four-byte fields without requiring
their numeric values in advance. The follow-up `apb_cursor_recheck.py` repeats
white/red/green against the **same process launch**. The saved first writable
page snapshot can be inspected with `apb_cursor_pointer_refs.py`; the live
reader then checks the pointer path and raw values while you move freely.
`apb_cursor_scan.py` remains an optional raw-value scan and now accepts any
observed integer from 0 to 65535. Earlier values 44, 88, and 119 were not
reproduced in this build; the verified values are white 130, green 48, red 99.
See [APB_CURSOR_MEMORY.md](APB_CURSOR_MEMORY.md) for the established pointer
path and the exact live-read command.

Example start from an elevated PowerShell in the repository root:

```powershell
powershell -NoProfile -File scripts/run_apb_cursor_unknown.ps1 -OutputDirectory '..\drayebr\build-rpm\snapshots\cursor-new-session'
```

After that capture, use the same-session candidate recheck. Once the live
readout identifies a field address, scan the saved pages for nearby pointers:

```powershell
powershell -NoProfile -File scripts/run_apb_cursor_recheck.ps1 -SourceDirectory '..\drayebr\build-rpm\snapshots\cursor-new-session' -Output '..\drayebr\build-rpm\snapshots\cursor-recheck-new-session.json'
powershell -NoProfile -File scripts/run_apb_cursor_pointer_refs.ps1 -SnapshotDirectory '..\drayebr\build-rpm\snapshots\cursor-new-session' -Target '0xYOUR_OBSERVED_FIELD_ADDRESS' -Output '..\drayebr\build-rpm\snapshots\cursor-pointer-refs-new-session.json'
```

The current product cursor path is `APB.exe + 0x3BC4230 -> object + 0x134` for
the verified executable hash. The finder is retained for future build changes;
a new candidate is not promoted into the product until its values and pointer
path survive a full restart.

## 2. Camera and projection

The saved table experiment is `scripts/run_apb_camera_capture_f8.ps1` with
`apb_camera_compare.py`. In the first action district, 65,536 observed table
entries contained no class-16920 camera object. Comparing level/up/down table
blocks produced a false pitch lead: level and up blocks were identical while
down belonged to another object. Keep the raw table captures for diagnosis;
do not promote that lead or call the observed table `GObjects`.

The active discovery path is one **whole writable-memory marker scan** per
district or process launch. It searches through EchoDriver for class 16920 at
object offset `+0x3C`, reads the candidate matrix at `+0x680`, position at
`+0x6C0`, and saves a client screenshot. A provisional camera shape has finite
values, matrix element 14 near `-9.99`, and element 15 near zero (absolute
tolerance 0.02). This was unique in the first three observed states. After the
next launch's action-to-social transition, the old action camera remained a
second shape match. A separate upward-pitch capture identified the responsive
social camera while the old action camera remained unchanged. Shape alone is
therefore insufficient to select the active camera. The scan now
reports the number of matches and fails visibly if it is not exactly one;
its complete raw report remains saved for diagnosis.

```powershell
powershell -NoProfile -File scripts/run_apb_camera_marker_f8.ps1 -OutputDirectory '..\drayebr\build-rpm\snapshots\camera-marker-new-session'
```

Validate the provisional address with **separate** F8 captures looking up and
down. Supply the new marker report and its reported object address. Each
capture verifies the same APB process identity, reads through EchoDriver before
and after the screenshot, and fails if the class marker changed.

```powershell
powershell -NoProfile -File scripts/run_apb_camera_candidate_f8.ps1 -MarkerReport '..\drayebr\build-rpm\snapshots\camera-marker-new-session\marker_scan.json' -Address '0xOBSERVED_OBJECT' -Label camera-up -OutputDirectory '..\drayebr\build-rpm\snapshots\camera-up-new-session'
powershell -NoProfile -File scripts/run_apb_camera_candidate_f8.ps1 -MarkerReport '..\drayebr\build-rpm\snapshots\camera-marker-new-session\marker_scan.json' -Address '0xOBSERVED_OBJECT' -Label camera-down -OutputDirectory '..\drayebr\build-rpm\snapshots\camera-down-new-session'
```

`scripts/run_apb_camera_refs_f8.ps1` scans exact pointers to a validated
same-launch camera candidate. `scripts/run_apb_camera_follow_f8.ps1` checks
those saved pointer slots after a district or character change in that same
launch. It records all outcomes and screenshots; zero camera targets is a
valid negative result, not a reason to choose another pointer silently.

```powershell
powershell -NoProfile -File scripts/run_apb_camera_refs_f8.ps1 -CandidateCapture '..\drayebr\build-rpm\snapshots\camera-up-new-session\capture.json' -OutputDirectory '..\drayebr\build-rpm\snapshots\camera-refs-new-session'
powershell -NoProfile -File scripts/run_apb_camera_follow_f8.ps1 -SourceReferences '..\drayebr\build-rpm\snapshots\camera-refs-new-session\references.json' -Label next-district -OutputDirectory '..\drayebr\build-rpm\snapshots\camera-follow-new-session'
```

Observed on 2026-09-30, all in APB PID 15616 with the verified executable
hash `3BECA02821F50A7B8FEF3E16923FA3FCA0D0B0DD49CC9622520EA500001E1640`:

| State | Camera object | Pitch check | Exact references | APB.exe references |
| --- | --- | --- | ---: | ---: |
| First character, action | `0x6531600` | down `-1`, level `~0`, up `+1` | 14 | 0 |
| First character, social | `0x112C3A080` | level `~0`, up `+1`, down `-1` | 22 | 2 |
| Second character, action | `0x1701E4B80` | level `~0`, up `+1`, down `-1` | 16 | 0 |
| After APB restart, second character in action | `0xA7F70B80` | level `~0`, up `+1`, down `-1` | 16 | 0 |

The 14 action pointer slots found no live camera after moving to social. The
22 social slots found no live camera after the character and district change.
Two APB.exe locations referenced the social camera only: module RVAs
`0x3CCBE38` and `0x3D24000`; they held other object pointers in the second
action state. One heap cluster had a reference at `0x170C84098`, then
`0x170C84090`, then `0x170C84088`. Its eight-byte shift is an investigative
lead, not a stable pointer. After a full APB restart (PID 18564), the same
whole-memory scan read 3,418 MiB with zero failed pages and found exactly one
shape match, `0xA7F70B80`. Separate up/down F8 captures of that object again
read `+1/-1` with matching ceiling/floor screenshots. Its 16 exact pointer
references had no APB.exe source and no exact source-address overlap with the
three pre-restart reference sets. This validates **scan rediscovery** across
restart, not a stable pointer or offset. Yaw, known-world-point projection,
and camera ownership remain unverified.

### Pointer-stabilization leads

All four camera objects have the same first eight bytes, a vtable pointer at
`APB.exe + 0x2B5EEA8`. This is a strong type check for a scanner, but the
vtable address does not itself locate the active instance. The 32-bit field at
object `+0x38`, immediately before class 16920 at `+0x3C`, reads `2`, `4`,
`6` across action/social/second-character action in one launch, then `2` after
restart. This pattern is consistent with camera-instance recreation and a
resetting sequence, but the field's meaning is unverified. Do not treat it as
an offset or choose the largest value without independent active-camera
checks.

The exact camera-pointer source at `0x170C84098`, `0x170C84090`, and
`0x170C84088` moves by eight bytes across the first three states. That is one
same-launch heap cluster; the old addresses are not valid after restart.
Tracing that cluster's owner and checking an analogous owner after restart is
the most concrete lead toward a stable root. A direct APB.exe pointer is not
supported by the captures: the two social-only module references did not
survive the next action state, and the restart references had no module source
in the scanned writable pages. The scan-based rediscovery path initially had
better evidence than any fixed pointer chain. The subsequent table-root
investigation below found a more promising module-rooted candidate.

### Current module-rooted table candidate

In PID 18564, the action owner chain was
`0x9E6D280 + 0x8 -> 0x9BC9300 + 0x310 -> 0x9BC9628 + 0x8 -> 0xA7F70B80`.
The observed class markers were 16863, 16865, 16998, then camera 16920.
After moving to social, all 20 saved references to the top owner failed to
reach that chain. A typed root scan found 481 top-type objects and zero
complete chains. That action chain is not a cross-district solution.

The shared pointer page at `0x168118000` contained the action camera at
`+0x88` and the responsive social camera at `+0x90`. Reverse scanning its
array-start address found a module reference:

`APB.exe + 0x3DBF110 -> 0x168118080 -> pointer entries`

The separate social table-root check at 11:06 on 2026-09-30 read this module
slot through EchoDriver and found one camera-marker entry in its first 128
slots: index 2, `0x9D21EB00`. The table pointer, entries, matrix and position
were unchanged across the screenshot. The screenshot showed an upward view,
consistent with matrix element 11 of `+0.8131444`. This camera had already
responded to the controlled upward-pitch check while the stale action camera
had not.

Adjacent 32-bit module fields at root `+0x8` and `+0xC` read 189 and 500.
They resemble count/capacity fields, but their meaning is not yet established;
the current tool explicitly scans only the first 128 entries. The table's
engine name, full bounds and active-camera selection rule remain unverified.
The return to Waterfront action at 11:20 used the same module RVA and table
address. Its first 128 entries contained one camera-marker entry at index 1,
`0x1991F2B00`, with matrix element 11 `+0.9999993`. The screenshot showed an
obstructing surface, so this capture alone does not visually establish the
camera direction. The table entries were unchanged across the capture. The
neighboring fields changed to 390 and 500, providing additional evidence for
the count/capacity hypothesis without establishing their meaning. This is a
successful social-to-action table lookup in the same launch. The separate
downward check at 11:23 reached the same camera at index 1 and read matrix
element 11 `-0.9999997`, against `+0.9999993` in the upward check. Its screenshot
showed the pawn from above, consistent with looking down. Both checks used
EchoDriver and completed with exit code zero. This verifies the new camera's
pitch response through the proposed module root.

The character-change check at 11:28 reached the EU character Malzel in
Financial, confirmed by the user and screenshot. PID 18564 and the module RVA
were unchanged; the table contained one camera-marker entry in its first 128
slots, index 1, `0x41E02A080`. Matrix element 11 read `+0.99999994` before and
after the screenshot, which showed the ceiling. The adjacent fields read
418 and 500. This verifies the table lookup after the character/region change;
the new character's camera has one upward view capture, not a separate
up/down response pair.

The full-restart check at 11:36 ran against new PID 19748, created filetime
134352127694529308, with the same executable SHA256. Resolving the same module
RVA found a new table address `0x162D64080` and a single camera-marker entry
in the first 128 slots at index 1, `0xA0B9C100`. Matrix element 11 was
`+0.99999994` before and after capture, matching the ceiling screenshot for
Malzel in Financial. The table entries were unchanged during capture; the
adjacent fields read 418 and 500. The report explicitly records
`source_same_launch: false`. This verifies lookup through the proposed module
root across one full restart, with both heap addresses relocated. The separate
downward capture at 11:38 reached the same table, index and camera. Matrix
element 11 read `-0.99999881` before and after capture, with a matching
downward screenshot. The up/down response check therefore passed after restart
through EchoDriver. Yaw and known-world-point projection remain unverified.

The F8 terminal must start after the new APB process is running: an existing
terminal holds the old process session and must not be reused across restart.
Do not hardcode the observed heap address
or index 1/2 as the stable location.

Evidence: `camera-restart-container-refs-20260930-110006/container_refs.json`
and `camera-restart-social-table-root-20260930-110636/table_root.json`, plus
`camera-restart-action-return-table-up-20260930-111615/table_root.json` and
`camera-restart-action-return-table-down-20260930-112233/table_root.json` and
`camera-character-change-table-up-20260930-112409/table_root.json` and
`camera-second-restart-financial-table-up-20260930-113615/table_root.json` and
`camera-second-restart-financial-table-down-20260930-113803/table_root.json`, under
`../drayebr/build-rpm/snapshots`.

```powershell
powershell -NoProfile -File scripts/run_apb_camera_table_root_f8.ps1 -SourceReferences '..\drayebr\build-rpm\snapshots\camera-restart-container-refs-20260930-110006\container_refs.json' -Label action-return-up -OutputDirectory '..\drayebr\build-rpm\snapshots\camera-table-action-return-up'
```

Run each view in a separate elevated terminal, arrive in the requested
district, hold the requested view, and press F8 with APB foreground. This tool
permits a different process launch only when the executable hash matches; it
resolves the module base afresh and reads the proposed module RVA. It does not
silently run a whole-memory scan when the table check fails.

### Live camera inspector

`scripts/run_apb_camera_live.ps1` launches a live diagnostic panel from a
completed module-table capture containing exactly one camera entry before and
after capture. The saved executable hash and verified module RVA must match the
running APB build; the PID and heap addresses may differ after a restart. The
panel resolves the current table through the module RVA. It displays derived
pitch/yaw, raw pitch component, position, module/table/camera addresses, table index, neighboring
unclassified fields, read duration and the raw 4x4 matrix. Memory bytes are
read exclusively through `Session.read` / EchoDriver. It does not move the
mouse or feed product targeting.

```powershell
powershell -NoProfile -File scripts/run_apb_camera_live.ps1 -Capture '..\drayebr\build-rpm\snapshots\camera-second-restart-financial-table-down-20260930-113803\table_root.json' -OutputDirectory '..\drayebr\build-rpm\snapshots\camera-live-new-session'
```

Refocus APB and level the view, then press F8 to begin. The panel updates every
200 ms by default; a second F8 in APB ends recording. Alt-tab explicitly pauses
reads and clears the current sample. Close the panel to exit. `session.json`,
`samples.jsonl` and the terminal `.log` retain identity, raw matrix/position,
derived values and status transitions. The panel remains visible on a fatal
error and closes with a failed exit after dismissal. A table or camera-slot
change clears the displayed values and logs `UNAVAILABLE`. The inspector then
rechecks the module table every 500 ms and resumes only after the same unique,
typed, shape-checked camera appears in two consecutive lookups. It audits the
first 128 entries every two seconds while live. Zero or multiple cameras remain
visibly unavailable; the inspector never chooses one silently. An unreadable
module root or EchoDriver failure stops the session. Restart the panel for a
new APB process. The table's full bounds and active-camera selection rule are
still unverified, so a unique typed entry is an observed diagnostic condition,
not proof of the active view target during every death or loading state.

For the observed matrix layout, the forward vector is `[m3, m7, m11]`, and the
horizontal right vector is `[m0, m4]`. Pitch is `asin(m11)`. Provisional yaw is
`atan2(-m0, m4)`, normalized to 0..360 degrees: zero means world +X, 90 means
world +Y. The horizontal-right derivation assumes the observed negligible
roll and remains usable near vertical pitch. A separate forward-XY yaw is
shown away from vertical for comparison; near vertical it is explicitly
undefined. These are derived diagnostic angles, not verified raw engine
rotation fields. The live feed fails visibly if the required matrix shape or
orientation assumptions fail.

For a controlled yaw check: stand still with the camera level, start the feed
with F8, hold still briefly, turn left horizontally and stop, return to the
starting heading, turn right and stop, then press F8 to finish. Compare yaw
changes with raw vector changes and pitch drift in the saved trace before
promoting yaw. Syntax validation and evaluation on existing saved captures
confirmed the derivation is internally consistent. The player later observed
the expected yaw switch when looking back; absolute heading and projection
checks remain pending. No new automated tests were added.

The first live run, `camera-live-yaw-check-20260930-114727`, recorded 639
samples between 11:47:51 and 11:50:04 Manila time in PID 19748. The user
reported that the live feed worked flawlessly. All successful samples used
table `0x162D64080` and camera `0xA0B9C100`. Yaw changed in both directions;
455 samples had pitch within five degrees of level. Across samples with a
usable forward-XY heading, the right-vector and forward-vector yaw estimates
agreed within 0.04781 degrees. The complete pitch range was approximately
-68.8623 to +23.0713 degrees, so this was free movement rather than a strictly
fixed-pitch trace. These observations support the live angle readout. Exact
turn direction/sign, independent absolute heading and world-to-screen point
alignment have not been measured against an external reference.

At 11:50:04 the pinned index-1 slot changed from `0xA0B9C100` to
`0x9BFA7B0`. The inspector visibly stopped and logged the error; it did not
read the replacement as a camera. The window was then closed. This is a
remaining stability issue: determine whether the original camera moved to
another index, was replaced, or changed with a view/district transition before
promoting the live lookup for continuous use. The user reports taking a
screenshot using Win+Shift+S at that point. A causal connection to the slot
change remains unverified. The fresh F8 table check
`camera-after-screenshot-table-20260930-115325` reached the same module RVA,
table `0x162D64080` and original camera `0xA0B9C100` back at index 1, before
and after capture. Its raw pitch component was approximately -0.1539.
The observed slot change was therefore transient rather than a permanent
replacement of that camera. The inspector now treats this kind of transition
as unavailable and attempts the verified module-table lookup again.

The 12:14–12:20 Manila-time live run on 2026-09-30 used PID 19764 and the same
executable hash. It saved 1,306 samples in
`camera-live-lifecycle-elevated-20260930-1215/samples.jsonl`. The module RVA
resolved to table `0x5216040`. The inspector acquired four successive cameras:
index 1 `0x9EE4AB00`, index 2 `0x14D054B80`, index 2 `0x162484B80`, and index 1
`0x16D1D0100`. At each of the three transitions, its log recorded unavailable
camera data, then a stable acquisition. The final camera remained readable
until the panel was closed. The terminal exited with code zero. These are
district changes according to the player. The player also reported clean live
readings through death and respawn, position changes when using ADS, changing
FOV, and swapping shoulders, and the expected yaw switch when looking back.
Those controls did not require a camera reacquisition in the trace. The log
records camera data and transition times; the in-game state and visual response
labels come from the player's observation. This validates the live diagnostic
through the exercised lifecycle, while the full table bounds and an independent
world-to-screen projection check remain open.

## 3. Other-player witness

`scripts/run_apb_players_f8.ps1` is separate because identifying another pawn
requires controlled movement evidence, not just the camera transform. First,
stand in the district where the mission will run, capture the desktop's local
baseline with F8, then start an open-ended local jump trace with F8. Jump twice
in place and press F8 after landing to end it. This local stage takes place
before the five-minute witness mission.

Next use the desktop account as a stationary observer and the laptop account
as a visible witness in the mission. The terminal prints when to start the
mission. The witness F8 sequence is:

1. Both accounts still, with the laptop witness visible: baseline.
2. Move **only** the laptop witness to a distinct nearby position and stop:
   moved capture.
3. Return **only** the laptop witness to its starting place and stop: returned
   capture.
4. Press F8 to start an open-ended witness trace. Jump twice in place on the
   laptop. After landing, press F8 on the desktop to end it.

```powershell
powershell -NoProfile -File scripts/run_apb_players_f8.ps1 -OutputDirectory '..\drayebr\build-rpm\snapshots\witness-new-session'
```

Each still phase saves raw table/header data and the first 0x100 bytes of every
readable table object, so later field analysis is possible even when the
current class-4741 marker proves incomplete. It also saves full candidate-position,
controller, and owner bytes, readable
position candidates, pointer links, a screenshot, and timestamps. The movement
report ranks objects by observed XY displacement and return distance. The
open-ended local and witness jump traces record XYZ values and report Z ranges;
they do not require pressing F8 in midair. These are **unclassified leads**. A moving
world object, an unrelated player, or a stale pointer can also appear in a
single phase. Confirm a witness lead against both controlled motions and the
screenshots, then establish its actual pawn type and client-visible player
identity relationship. Repeat that relationship after respawn and an
APB restart before it can supply Memory targets or body bounds.

### First two-account capture on 2026-09-30

`witness-two-account-20260930-ready` records PID 19764, created filetime
134352150975111460, with the verified executable hash. The desktop local trace
saved 40 rounds with no read errors, but all 846 class-4741 candidate positions
were unchanged. It does not establish the desktop character's position; whether
the requested desktop jumps occurred inside this trace needs player confirmation.

The witness still captures saved 849, 825 and 808 positions. All 808 addresses
shared by baseline/moved/returned had zero XY displacement from baseline to
moved. Screenshots show KENOLANGOT at the same spot in baseline and moved, then
at a different spot in returned. Thus the controlled away-and-return sequence
was not captured as labeled and must be repeated.

One lead, `0x154B77C00`, was present in all four captures with marker 4741 and
vtable `APB.exe + 0x3142FB0`. Its baseline and moved positions were
`[151725.984375, 151600.984375, 94]`; returned was
`[151874.015625, 151163.015625, 84]`. The laptop jump trace saved 69 rounds
without read errors and two Z peaks at elapsed 4.000 and 5.734 seconds. The
largest Z rise was 160.15016, from 84 to 244.15016, returning to 84. Each XY
axis varied by only 0.015625. This is a jump-responsive position lead, with no
verified pawn or controller identity. No exact reference to it was found in
the saved first-0x100 object prefixes or the captured controller/owner bodies.

The original collector exited normally despite the missing movement evidence.
Its completion check has been corrected: unchanged shared XY candidates now
produce `movement_not_observed` and a visible failure after preserving the jump
trace. A trace with no Z changes explicitly records `jump_not_observed` and
prints that it has not identified the subject's position. No new automated
tests were added; Python syntax validation passed.

The repeat in `witness-two-account-repeat-20260930-1` completed the controlled
witness movement. Of 672 positions shared by the three still captures,
`0x154B77C00` alone changed XY: 232.14738 units away and 20.25773 units from
baseline after returning. The baseline/moved/returned screenshots show
KENOLANGOT in the corresponding positions while the desktop remains still.
The 63-round witness jump trace again isolated the same address: Z rose from
84 to 242.20636 and returned to approximately 84, while each XY axis varied
only 0.015625. There were no jump read errors. The repeat local trace saved 90
rounds, but none of its 718 candidate positions changed. The local position
remains undiscovered in this table subset; this is not a validated local pawn.

`run_apb_player_refs_f8.ps1` follows an explicit movement/jump-confirmed address
in the same APB launch. It verifies the saved vtable and position marker, scans
exact references through EchoDriver, rechecks the pointer slots, and saves
surrounding pages plus a screenshot for owner analysis. All resulting owners
remain unclassified. It does not rediscover another address when validation
fails. Example from an elevated repository terminal:

```powershell
powershell -NoProfile -File scripts/run_apb_player_refs_f8.ps1 -SourceDirectory '..\drayebr\build-rpm\snapshots\witness-two-account-repeat-20260930-1' -Address '0x154B77C00' -OutputDirectory '..\drayebr\build-rpm\snapshots\witness-position-refs-20260930-1'
```

After the laptop account reconnects, repeat its movement and jump captures;
the previous position address may describe a stale object. The player collector
accepts `-WitnessOnly` for this explicit repeat, records the selected stage in
`session.json`, and starts at the witness baseline. It does not repeat the local
trace or claim to validate the desktop pawn. A reconnect run was started in
`witness-reconnect-20260930-1` after KENOLANGOT disconnected for inactivity.
The earlier reference scan stopped without a completed `references.json`;
its partial scan log remains in `witness-position-refs-20260930-1.log`.

The reconnect capture completed with a new moving/jumping position lead at
`0x154B36C00`, while PID 19764 remained unchanged. This supports recreating the
remote position object after reconnect; its old heap address must not be reused.
The new lead moved 103.39125 units, but its returned position was 236.33530 units
from baseline. Screenshots likewise show a different return spot, so the
away-and-return validation failed. The 56-round jump trace showed approximately
165.31444 units of Z range and nearly fixed XY, without read errors. The reference
scanner's return check rejects this evidence. The collector now also reports
`return_not_observed` and fails visibly after saving the jump trace when no
moving candidate returns closer to baseline.

The next capture, `witness-reconnect-return-20260930-1`, visually shows
KENOLANGOT moving away and returning to the original indoor spot. The
class-4741/+0x88 collector nevertheless found no changing position, and its
63-round jump trace was stationary. The previous lead `0x154B36C00` was absent
from baseline, then present with an unchanged position and negative Z in the
later captures. Its continued readability is not evidence of a current player.

Comparing all saved object prefixes, across markers and float offsets, found
two controlled movement patterns in the same captures:

| Object and XYZ offset | Marker / vtable RVA | Move XY | Return XY |
| --- | --- | ---: | ---: |
| `0x14E9B5940 + 0x68` | 18220 / `0x2BFB8A0` | 592.14187 | 0 |
| `0x237E23900 + 0xD0` | 294 / `0x2AE68D8` | 591.43859 | 78.83567 |

Their markers and vtables were stable across all three still captures. These
are explicit field leads, not classified pawn/controller objects. The derived
`controlled_position_leads.json` records the source captures, offsets, raw
positions and motion measurements. The first lead's positions were
`[149615,151135,204]`, `[149049,151309,204]`, and `[149615,151135,204]`.
This demonstrates that the class-4741/+0x88 filter missed movement present in
the selected table's captured data. No automatic alternate reader was added.

`run_apb_position_leads_f8.ps1` accepts this explicit manifest for a targeted
laptop jump trace. It checks the same APB identity and each lead's vtable and
marker, records the specified XYZ field every 100 ms, and saves raw before/after
prefixes, screenshots and sample-gap diagnostics. F8 starts and stops the trace;
only the laptop jumps. Type/read errors stop visibly. This trace has been
started in `witness-targeted-jump-20260930-1`; it failed its first header check.
The independently inspected second lead completed a trace in
`witness-targeted-second-jump-20260930-1`: 141 rounds, maximum sample gap
0.110 seconds, Z range 203.46217. This confirms jump response at +0xD0,
but does not establish pawn identity.

### Death during the reference scan

`witness-field-root-refs-20260930-1` completed its reference search and saved
101 context pages, then failed its final position-header check. The user
confirmed killing and respawning KENOLANGOT during the scan. Offline recovery
found 27 exact references remaining in those pages. The scan formerly wrote
its JSON only after final validation; it now preserves separate before,
reference, slot and context reports as each phase completes, and writes an
explicit validation_failed final report when final capture or type checks fail.

The recorded typed owner hypothesis is `0x15D15F400`, vtable RVA `0x2B65A80`,
marker 6959. Saved +0x708 and +0x768 fields both reference the former position
lead `0x237E23900`. Immediate EchoDriver inspections after respawn retained
the owner's header but found the target's vtable changed to heap address
`0x237E20800`; retained XYZ and marker 294 do not make this target valid.

`apb_player_owner_inspect.py` is an explicit, same-launch diagnostic of this
recorded hypothesis, not a general player resolver. Its reports are in
`witness-owner-inspection-20260930-{1,2,3}`. The module contains one exact
reference at RVA `0x3CE29D0`. Neighboring slots contain numerous heap pointers:
this is an inline table entry, not a proven stable root. The explicit module
window survey `[0x3C00000,0x3DB0000)` inspected 56,406 unique pointers with
779 failed headers and found 18 owners matching this header. None supplied a
valid replacement of the recorded position type through those two fields.
These offsets and objects must not be promoted to the product resolver.

## Promotion gates

### Explicit district-wide diagnostic (mission filtering deferred)

The user accepted enumeration without mission membership filtering. This does
not establish that the observed inline table is a district player roster.
`apb_district_candidates.py` records the explicit module window
`[0x3C00000,0x3DB0000)`, deduplicates its pointers, preserves raw table and
header surveys, and checks two previously movement/jump-responsive types:
vtable RVA `0x3142FB0`, marker 4741, XYZ +0x88; and RVA `0x2AE68D8`,
marker 294, XYZ +0xD0. Other player types may still be missing; these are
unclassified candidates, not a claim of complete district population coverage.

F8 starts a 120-second recording; F8 stops early. Header surveys repeat five
seconds after each completes, with candidate samples every 200 ms between
surveys. The JSONL records additions, removals, invalidations, current camera
matrix, client dimensions and projected points. Read/type failures never reuse
a candidate's old position. Camera unavailability is explicitly logged and no
projection is emitted. Candidate positions and projection remain diagnostic;
this command does not select targets or send mouse movement. The summary
reports actual maximum sample gap, including survey time.

```powershell
powershell -NoProfile -File scripts/run_apb_district_candidates.ps1 -OutputDirectory '..\drayebr\build-rpm\snapshots\district-candidates-new'
```

The publisher's [June 2024 matchmaking description](https://www.gamersfirst.com/apb/news/2024/6/7/matchmaking-2.0)
describes backup/escalation adding players to mission teams. The
[Mobilize description](https://www.gamersfirst.com/apb/news/2024/12/12/mobilize)
describes cross-district matchmaking. These support investigating mission team
membership separately from district residency, but publish no native memory
offsets or table layout.

### Source validation requirements

The extensive manual run completed with 1,275 samples, 76 surveys, 21 annotation
files (17 positive labels, 4 exclusions) and 16 visual frames. The user reported
Social/Financial transitions, gun-model projections, loss/replacement of addresses
when moving away and returning, and occasional simultaneous old/new points.
These are evidence of position/component lifetime changes, not stable identity.
Camera-unavailable intervals were logged during transitions with no projections;
projection resumed afterward. Neither complete travel coverage nor uninterrupted
camera availability is claimed.

All 17 positive annotation events (15 distinct addresses) share the exact
relation: position vtable RVA `0x3142FB0`, marker 4741, XYZ +0x88; pointer
+0x178 targets vtable RVA `0x2AAEFA0`, marker 5778. None of the four negative
examples contains that typed link. Saved `annotation_review.json` and
`player_link_hypothesis.json` record this comparison. The linked +0xD0 pointer
also changes in repeated same-position examples, so it has not established
persistent identity across component replacement.

Offline checks against same-survey headers reduced 865 first-type positions
to 8 structure matches in survey 13; survey 73 reduced 9 to 3. Targets absent
from this diagnostic table window were not treated as negative proof.
`structure_filter_surveys.json` records matches and raw flags without assigning
death semantics. `-Overlay -StructureFilter` is an explicitly selected
experimental diagnostic: it reads the +0x178 target directly, requires its
exact vtable/marker, and rechecks the link during every sample. The UI labels
these as experimental structure matches, not verified player identities.
The original broad raw surveys are still retained for auditing. Zero matches
does not activate another coordinate source. Human/NPC distinction, weapon
exclusion beyond the small labelled set, unique-player grouping, lifecycle
eligibility and a stable player module root remain unverified.

The live structure-filter run `structure-visual-candidates-20260930-1`
completed with 361 samples, 16 surveys, five visual frames and six positive
operator annotations. Every survey retained seven structure matches from
642–799 broad position candidates; each of those seven addresses was sampled
361 times. The maximum sample gap was 1.625 seconds, including surveys.
The user confirmed annotating all other players and leaving their own character
unlabelled. In `visual-0004.png`, the unlabelled point `0x99A09800` projects
onto the visible local character. The six labelled addresses are `0x9DF45000`,
`0x16E335800`, `0x2B7C7F800`, `0x9DF46C00`, `0x9DF44800` and `0x9DF45C00`.
This is a successful character-focused visual validation in this scene.
There were no additional structure matches in this recording; it does not
establish cleanup across streaming/death, a persistent player identity, or
exhaustive NPC rejection. Local exclusion must use a verified relationship;
the visually identified local address is evidence for this recording only.

#### Current local ownership and reciprocal component membership

`apb_identity_inspect.py` preserves bounded typed bodies and array probes through
EchoDriver. Its probe extents are not verified object sizes: strings or links
from neighbouring allocations must not be interpreted as owned fields. In
particular, pawn +0xD0 targets vtable `0x3176770` / marker 3256; apparent names
farther into those probe extents were neighbouring UI allocations, not verified
player identities. The +0x128 array contained the same three shared objects for
every character and did not establish current component membership.

The September 30 captures established the following current local chain:

```text
camera +0x2E0 -> world (vtable RVA 0x3161E78 / marker 430)
world +0x358 -> controller (vtable RVA 0x2A7F158 / marker 998)
controller +0x790 -> pawn (vtable RVA 0x2AAEFA0 / marker 5778)
pawn +0x3E0 -> the same controller
```

The chain agrees across immediate captures and resolves the previously visually
identified local pawn. The supplied controller +0x790 offset therefore now has
matching live evidence, with a different current controller marker. Each of
seven pawns in `identity-inspect-20260930-5` had its projected position component
in its +0x140 pointer/count/capacity array; the local array also contained a
second component of another type. Pawn XYZ +0x88 agreed with the component XYZ,
subject to sequential live reads. Array ownership, fingerprints and traversed
pointers are checked again before accepting each sample.

`-Overlay -StructureFilter -OwnershipFilter` explicitly selects this diagnostic
implementation. It requires reciprocal current component membership, projects
canonical pawn +0x88 XYZ, excludes the local controller's pawn, and groups by
current pawn pointer plus its raw fingerprint. Multiple qualifying components
under one pawn are withheld with visible ambiguity counts and diagnostic logs;
there is no arbitrary component selection. Missing local ownership clears the
overlay and logs an unavailable state. Each frame is rebuilt from current
reads; no past projection or address is retained as a player identity.

Live `identity-inspect-20260930-6` validation completed successfully: two current
matches, one local excluded (`0x99A09800`), one remote retained, zero ownership
errors and zero ambiguous groups. This validates current ownership filtering;
it does not establish persistent account identity across pawn replacement or
complete cleanup of retained dead pawns. Streaming, local/remote respawn and
district transitions still require the updated overlay recording. These changes
remain in diagnostic scripts and have not been promoted to `src/apb_atlas.rs`.

The subsequent `ownership-district-candidates-20260930-1` recording completed
with 1,209 projected samples, 79 surveys and 15 saved comparison frames. The
user reported district travel and death/respawn reacquisition as reliable, but
vehicle entry could remove individual projections or the entire overlay.
The logs contain 161 ownership-unavailable events and 116 camera-unavailable
events; the 39.344-second maximum projected-sample gap means this was not
uninterrupted projection availability.

158 ownership failures came from controller +0x790 selecting vehicle type
`0x2B29FB8` / marker 6064 at `0x318B0B40` or `0x77F26200`. The old resolver
requires the on-foot pawn type and therefore deliberately suppresses all points.
Saved character bodies during these episodes retain canonical XYZ +0x88 that
changes while driving, but have an empty +0x140 component array and a +0x178
attachment to the possessed vehicle. The +0x3E0 controller link becomes zero.
After exit, the attachment disappears and component membership returns. Thus
an empty component array is not proof that a player has disappeared or died.

`run_apb_vehicle_capture.ps1` / `apb_vehicle_capture.py` capture four explicit F8
stages (on foot, driver seated stationary, driver after moving, exited). Each
stage saves module headers, bounded 0x3000 character/vehicle/controller bodies,
possession before/after checks and an APB screenshot. This evidence is needed
to validate driver/occupant ownership before revising the runtime filter;
passengers sharing a vehicle must not be mistaken for the local character.
The current character-only ownership mode still reports vehicle possession as
unavailable; no unverified vehicle identity rule has been substituted.

The four-stage `vehicle-ownership-20260930-1` capture completed with zero body
errors and unchanged controller possession in all four stages. Controller
`0x38D484080` possessed character `0x39216F040` on foot, vehicle `0x77F26200`
while driving, then the character again after exit. Controller +0x1380 and
+0x1388 both retained that character in all stages. While driving, character
+0x178 pointed to the vehicle and vehicle +0x3E0 pointed to the controller;
character +0x3E0 was zero. Vehicle +0x350 also pointed to the character after
exit and is not treated as a driver link (it overlaps the observed pawn-list
link offset). Current driving ownership therefore requires agreeing controller
character fields and reciprocal attachment/controller links, not +0x350.

The revised `-OwnershipFilter` directly surveys exact character pawn type
`0x2AAEFA0` / marker 5778, and continues using canonical pawn XYZ +0x88 for
every state. On foot it requires a registered +0x140 component with a reciprocal
+0x178 link; the recorded component types are `0x3142FB0` / 4741 and
`0x2B4B7A0` / 6683. While attached to the explicitly observed vehicle type
`0x2B29FB8` / 6064, an empty render-component array is accepted as a vehicle
state. Self-exclusion compares the resolved local character, not all characters
sharing that vehicle. Unknown possession types still fail visibly.

The first post-exit live validation correctly failed because only component
type 6683 remained registered, exposing the overly narrow 4741 requirement.
After adding the recorded reciprocal 6683 type, `vehicle-fix-validation-20260930-2`
completed with one current character, one local exclusion and zero errors.
No remote players were present in that immediate validation. Live driving,
other vehicle types and passenger behavior still require the revised overlay
recording; persistent account identity across pawn replacement remains unproven.

`fresh-launch-ownership-20260930-1` completed successfully on APB PID 17324:
652 samples, 32 surveys, 12 comparison frames, 295 on-foot samples and 357
local-driving samples. One focus-loss pause was followed by a logged resume.
The user confirmed consistency across restart and accepted pawn detection as
complete for the diagnostic workflow. This does not promote the implementation
to the Rust product or establish persistent account identity.

#### Experimental mission classification: September 30 capture

`mission-labels-20260930-2` supplied 14 role-label events across 11 pawn
instances and two observed mission tokens (`0x8000071C`, `0x80000720`). The
operator confirmed the final pawn `0x16EAB90C0` was opposition, correcting the
simultaneous teammate key event in annotation 0012. Raw annotations remain
intact; `mission_role_review-confirmed.json` records the explicit correction.
Role labels take precedence over the operator's delayed in-mission state mark.

Candidate fields are typed character pawn `+0x990` (full 32-bit mission token)
and `+0x994` (side). Same active token and same side predicts teammate; same
active token and opposite observed side predicts opposition. Differing active
tokens predict outside this mission. Only observed active sides 0/1 with the
token's high bit set are accepted. Token `0xFFFFFFFF` or observed inactive side
`0x80`, `0xC9`, `0xD9` indicates inactivity. Mission end retained the old token
with side `0x80`, so token equality alone is insufficient. Unobserved states
remain visibly unknown and logged. Fields `+0x704`/`+0x9A0` also matched an
outside-mission player's opposition faction and cannot establish mission role.

The hypothesis fits 14/14 corrected label events; these are discovery data,
not independent validation. Token semantics, reversed local side, concurrent
missions, mission end and new opponents require live confirmation. Inline
module slots around RVA `0x3CDDD78..0x3CDDF08` and `0x3D0DF50..0x3D0E0C0`
are observed references, not a proven global roster root.

`-MissionClassify -MissionLabels -OwnershipFilter -StructureFilter
-ManualFilter -Overlay` enables the experimental diagnostic. F4 toggles
opposition-only display; recording retains all points. Initial display shows
all roles. Manual labels remain separate from predictions, and disagreement
is displayed. Each mission-field read checks pawn type/instance and repeats
the read; the local fields are rechecked after sampling the remote pawns.
No faction substitution or production resolver change is made.

Reproduce the captured-data review with a new output filename (existing
reports are deliberately protected against replacement):

```powershell
python -B scripts/apb_mission_review.py ..\drayebr\build-rpm\snapshots\mission-labels-20260930-2 ..\drayebr\build-rpm\snapshots\mission-labels-20260930-2\mission_role_review-new.json --correct-last-pawn-opposition
```

#### Mission ground-truth recording

`-MissionLabels` adds F1 outside-mission player, F2 teammate, F3 opposition;
F4 remains a generic player label, F5 cycles overlaps, F6 hides and F7 undoes.
Ctrl+F2 marks the operator in mission; Ctrl+F3 marks outside mission. Ctrl
state annotations do not also label a player. Each state annotation begins a
new operator context epoch and clears old mission-role labels; the epoch is
not a memory-derived mission ID. State snapshots preserve the local character,
controller and world bodies. Selected-pawn annotations preserve local/remote
bounded 0x3000 bodies, first-0x1000 pointer-field contexts, type headers and
the original surveyed module-slot RVAs. Probe extents are not object sizes.
Without `-MissionClassify`, labels alone do not enable automatic classification.

Overlay recording controls are F8 start/stop/save, F9 comparison frame,
F10 pause/resume, F11 full exit. F8 stop saves the recording and returns to
idle; the next recording has a separate `recording-NNNN` directory. F11
closes the diagnostic and overlay, including from idle or focus/manual pause.
Paused recording emits no samples and rescans before resuming. The launcher
must omit PowerShell `-NoExit` for the shell to close after full exit.

Compare known teammate, opposition and outside-mission labels before, during
and after an opposed mission, then repeat with new opponents. Faction alone
does not establish mission opposition. APB's publisher describes separate
backup/escalation matchmaking in its
[matchmaking update](https://www.gamersfirst.com/apb/news/2024/6/7/matchmaking-2.0).
The UE3 primary source contains replicated player identity and a Team link in
[PlayerReplicationInfo](https://raw.githubusercontent.com/bakerm6/Src/master/Engine/Classes/PlayerReplicationInfo.uc);
these guide relationship discovery but do not supply APB native offsets.

The user requested manual exclusion in a Social district (possibly EU).
`-Overlay -ManualFilter -Seconds 1800` enables explicit operator annotation:
aim the overlay's red centre reticle at a candidate within 24 client pixels;
the selected address receives a red ring. F5 cycles overlapping points,
F6 hides only the selected instance, F4 labels it as a player (green), and
F7 restores the previous label. F9 saves the filtered view and raw point list;
F8 stops. Nonexcluded points are unlabelled, not presumed human players.

Each label saves its 0x800-byte object body, module table slot RVAs, current
projection/camera, and bounded read-only pointer context. Exclusions are applied
only to the same address and raw instance fingerprint (vtable plus fields
+0x24..+0x3F). The field meanings remain unverified; this is an audit fingerprint,
not proof of object generation or ownership. No labels are loaded across launches
or generalized into runtime player eligibility. New/different fingerprints are
unlabelled. All candidates remain in the raw recording, including hidden points,
so negative examples and undo history are retained. A changed instance rejects
the annotation visibly. Positive and negative labels can guide subsequent owner
and collection tracing; they do not establish a stable module root by themselves.

`social-visual-candidates-20260930-1` completed the visible click-through
overlay: 359 rounds, 21 surveys, 12 saved visual frames, exit 0. Maximum
sample gap was 2.828 seconds, including survey pauses. Visual inspection found
cyan markers at the desktop character's hip and at named characters Tazanna,
pawjob, Vodica, Careless and mimsuwu. The mimsuwu point retained address
`0x15212CC00` while aligning with the rooftop character in frames 9 and 11
from different viewpoints. These are visual correspondences, not decoded
in-memory names or verified player ownership.

`visual_correspondence_leads.json` preserves the exact frame, point, manually
selected screen anchor and saved candidate body for each match. Anchors were
selected from the displayed markers to recover their addresses; distances to
these anchors are not independent projection-error measurements. All six
distinct visually matched objects have marker 4741 and vtable RVA `0x3142FB0`.
Their addresses are desktop `0x14ED06400`, Tazanna `0x1F10CB000`, pawjob
`0x15F903C00`, Vodica `0x15B03FC00`, Careless `0x1624F6C00`, and mimsuwu
`0x15212CC00`.

The same type also projects onto scenery, kiosks, bar furniture and other
world objects. Frame 1 contains 482 on-screen candidate points. Thus the
type/marker filter does not identify human pawns by itself. This run supports
live projection of character-corresponding position sources in Social; it
does not establish complete population coverage, body bounds, player identity,
mission membership or death/respawn ownership. No address-range, movement-only
or object-number heuristic was added to remove scenery.

For visual Social validation, `-Overlay -Seconds 600` displays Windows
click-through points aligned to the APB client. F8 starts after district entry
and stops; F9 saves a screenshot and the corresponding candidate positions,
camera matrix and projection values. Cyan points use +0x88/marker 4741;
yellow points use +0xD0/marker 294. Address labels identify exact candidates
for subsequent pointer analysis. The overlay explicitly states that NPCs,
vehicles and stale objects may appear. It neither names players nor classifies
opponents, selects targets or sends input.

The points are cleared during surveys and camera unavailability, and hidden
when APB loses focus. Refocusing APB triggers a new table survey. Screenshots
include the diagnostic markers for visual alignment checks; JSON retains the
sample timestamp separately from screenshot timestamps. The Windows overlay
requires windowed/borderless presentation if exclusive fullscreen prevents
composition. There is no alternate renderer or capture fallback.

`district-lifecycle-20260930-1` completed all four controlled stages and exited
0: 293 rounds, 19 full-body surveys, maximum sample gap 2.094 seconds. Stage
surveys are 0 (alive baseline), 5 (alive after motion), 13 (dead) and 18
(respawned after motion). The user clarified the laptop respawned far away on
the right side of the map. Screenshots show the desktop view remained fixed;
the laptop is not visible in the dead/respawned stage screenshots. User stage
labels therefore supply the death/respawn timing, not visual identification.

Eight candidates changed position by at least five world units. One substantial
motion lead, `0x2BB04E400` (vtable RVA `0x2AE68D8`, marker 294), retained its
header through all four stages. Its +0xF8 and +0x420 pointers referenced
`0x2BB065600` before death, then became zero in survey 10 and stayed zero.
The referenced object had vtable RVA `0x2AFF240`, marker 1062; its 64-bit
header field +0x10 changed from `0x110100000` to `0x2000000110100000` by
the dead stage. The position object's +0x84 field changed from 1 (or 9 during
movement) to 4 and remained there. `lifecycle_link_evidence.json` preserves
the sequence. None of these fields has verified ownership, death, health or
player-identity semantics. No eligibility rule was added from this one event.

The linked type is now explicitly captured in separate full-body files, without
using it as a position source. Immediate survey `district-linked-bodies-20260930-1`
saved 1,908 position bodies and 23 linked-type bodies with no body failures.
Twenty-three objects in a two-human district cannot establish human identity.
A distant respawn may require different replicated/pawn relationships or may
fall outside the selected module-table window; the missing replacement has not
been attributed to either cause. Camera distance and movement must not be used
to infer identity or exclude an otherwise verified distant player.

For controlled lifecycle evidence, run `run_apb_district_candidates.ps1` with
`-Lifecycle -Seconds 600`. Four F8 stage captures record alive baseline, alive
after laptop-only movement/jumps, laptop dead before respawn, and laptop
respawned after movement/jumps. Keep the desktop stationary and APB foreground.
The console prints each requested action only after its preceding full-body
survey and screenshot finish. Each stage JSON links its labeled screenshot to
the exact full-body survey sequence. Continuous sampling and five-second
surveys run between stage markers. A time limit before all four stages fails
visibly with completed files retained. The stage labels are operator evidence,
not inferred pawn state or validated player identity.

The first completed district recording, `district-candidates-20260930-2`, exited
successfully with 351 sample rounds and 20 surveys. Across those surveys it
observed 2,400 distinct addresses (1,834 of the first type and 566 of the
second), rejected 128 changed-type samples, and retained camera address
`0x1617D4B80`. Only 11 addresses changed any position axis by more than one
world unit. Maximum sample gap was 1.546 seconds; survey pauses mean this is
not yet a high-rate jump diagnostic. Derived results are in `analysis.json`.

The user confirmed moving/respawning both characters. The camera also changed
position, so camera distance does not independently identify the laptop.
Candidates `0x159F28800` and `0x2B7C7F800` stopped changing at approximately
20.67 and 25.58 seconds respectively, remained typed and projectable until
76.47 seconds, then were invalidated. Their saved 0x40 headers were unchanged
while their positions were frozen. Thus header checks alone are insufficient
for live-player eligibility. Stillness must not be used as a death classifier:
an alive player may stand still. Full candidate bodies (0x800 bytes) are now
preserved at every survey for independent lifecycle/ownership analysis in the
next recording. No pawn status or health offset has been inferred or promoted.

The user subsequently confirmed only two human players were in this district,
plus NPCs and vehicles. Eleven moving objects therefore cannot be interpreted
as eleven unique players. Multiple components, NPCs, vehicles and retained death
objects remain possible explanations. User-reported morph targets on characters
and cars do not distinguish player identity. UE3
[`SkeletalMeshComponent`](https://raw.githubusercontent.com/bakerm6/Src/master/Engine/Classes/SkeletalMeshComponent.uc)
supports morph targets and multiple meshes sharing animation within an actor;
these are engine relationships, not identified APB native types. Investigate
actual actor ownership, group components under their actor, and verify the two
human identities independently before describing a district player roster.
`-InspectOnly` preserves a current full-body survey immediately without requiring
another movement sequence or foreground screenshot.

`district-bodies-20260930-1` completed an immediate survey with 1,793 candidate
bodies, 781 failed table headers and zero candidate-body failures. Only one of
the eleven previously moving addresses remained in this snapshot, demonstrating
that address lists cannot be retained as stable identities. Saved
`moving_candidate_links.json` lists exact pointer fields whose target headers
were present in the same table survey; it does not assign ownership semantics
to those fields. Ownership targets outside this diagnostic window may be absent.

- Camera: pitch and yaw response repeats, image and memory timestamps are
  close enough for a visual check, several known world points project onto
  their visible objects, and a pointer/ownership path rediscovers the camera
  across launch and view-target changes.
- Local pawn: an in-place jump produces a repeatable Z rise and return while
  XY remains stable; the local player/controller/pawn relationship is verified
  independently of camera distance.
- Witness pawn: only the laptop moves in the controlled phases; its candidate
  moves away, returns, jumps, corresponds to screenshots, and has validated
  client-visible player identity and current pawn ownership. Remote controller
  existence is not assumed; see the UE3 source relationships in
  `MEMORY_COORDINATES.md`. Top/bottom screen bounds need their own
  checked world-space source before Y movement uses them.

The existing Memory implementation is still diagnostic until these gates are
met. The F8 tools collect evidence; they do not silently substitute another
object or coordinate source when a check fails.
