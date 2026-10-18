# Laptop-player health and stamina discovery

This is a new F8-paced diagnostic for the laptop-loaded character in your group.
It uses the desktop APB process and EchoDriver memory reads. The laptop HUD is
the operator's reference; this tool cannot read or screenshot the laptop.
It also captures your desktop character as a separately labelled local control.
There is no targeting, shooting or generated game input.

## Start and select

Use an administrator PowerShell terminal, with APB loaded into the same district
as the laptop character. Both characters should stay alive, on foot, throughout
this scan. Windowed/borderless presentation is required for the diagnostic overlay.
Use a new output path whose parent exists:

```powershell
powershell -NoProfile -File scripts/run_apb_health_stamina_scan.ps1 -OutputDirectory '..\drayebr\build-rpm\snapshots\health-stamina-laptop-new-session'
```

The scan discovers the local controller through the validated camera/World route
and enumerates character pawns through World +0x48C and Pawn +0x350, including
the validated vehicle node type when traversing the list. It checks every edge,
type and fingerprint and rejects unsupported types, cycles and changed links.
It never substitutes a module-window survey for a failed rooted traversal.

After both characters load into the same district, focus desktop APB and press
**F8 to begin selection**. Camera and local ownership are resolved only after
that explicit readiness press; no camera route is replaced if resolution fails.
On the desktop, aim the reticle at the laptop character's **cyan body-center
marker**. A nearby selected marker becomes green. **F5** cycles overlapping
markers within 24 pixels of the reticle; **F8** pins the green marker.
You identify the laptop group member visually; the tool does not infer an account
name or group membership from unknown fields. The next baseline F8 confirms the
green marker belongs to that laptop character. If you selected the wrong marker,
exit with **F11** and restart into a new output directory.

The exact selected instance stays pinned. Target collision-component backlinks
and World membership are checked before/during each capture. Either character
respawning or changing district stops the scan with completed phases preserved.
No pawn is automatically replaced. A retained corpse is not classified as alive
by these checks: keep the laptop alive and use its HUD to establish each state.

## F8 phases

The overlay and terminal show each instruction. Keep **desktop APB foreground**
when pressing F8, while the laptop operator prepares the state. There is no
countdown or automatic advance. Wait for `CAPTURE_COMPLETE` before preparing
the next state. Each press captures immediately on its key-down edge.
While capturing, the overlay explicitly displays `CAPTURING`; F8 presses are
discarded. After capture, F8 must be released before the next phase is armed.
A held key cannot trigger consecutive phases.

| Phase | Laptop state to establish before F8 |
|---|---|
| baseline_full | Full health and stamina; stationary |
| health_damaged | Partial health damage; stamina full |
| health_more_damaged | Further health damage; stay alive; stamina full |
| health_recovered | Both fully recovered |
| stamina_partial | Partly deplete stamina using less-lethal damage, e.g. PIG; health full |
| stamina_low | Further less-lethal stamina damage; health full |
| stamina_recovered | Both fully recovered |
| health_repeat_damaged | Damage health again; stamina full; stay alive |
| health_repeat_recovered | Both fully recovered |
| stamina_repeat_low | Less-lethal stamina damage again; health full |
| stamina_repeat_recovered | Both fully recovered |

Press F8 promptly for depleted states, before regeneration changes the laptop HUD.
Health means damage to health, not stun damage. Stamina here means the resource
affected by less-lethal/stun damage, not sprint duration. Avoid mixing the two resources
or unrelated equipment/stance changes. Keep the desktop character stationary,
healthy and rested as the control where practical. Observed HUD percentages or
unexpected events can be supplied afterwards to interpret the saved timestamps.

F8 is ignored while another app is foreground. F11 exits and records a stopped
scan as incomplete; it does not pretend that missing phases were collected.
The launcher refuses to start a second copy listening for F8. For an explicit
restart, add `-RestartExisting` and choose a new output path. This stops only
Python processes whose command line names this exact scanner script and leaves
their saved files intact; an interrupted phase can remain incomplete.

## Captures and interpretation

Every phase preserves three sequential samples, spaced by at least 40 ms after
each sample's reads. Actual read duration is recorded; this is not a promised
fixed-rate sampler or atomic process snapshot. Each sample saves:

- The laptop pawn's first 0x3000 bytes, read before speculative related objects.
- The local pawn's first 0x3000 bytes, separately labelled as a control.
- Bounded one-hop objects reached by plausible pointers at four-byte steps in
  each pawn's first 0x1000 bytes and by its registered-component array.
- Object types, instance fingerprints, pointer field locations, timestamps,
  SHA-256 hashes and any failed speculative pointer reads.

Each 0x3000-byte pawn body is obtained through three requests of at most
4096 bytes, respecting EchoDriver's request limit. A failed request reports
the exact address, requested length and driver error, and saves a traceback.

Related objects require a vtable inside the verified APB image and unchanged
fingerprints during capture. There is a 64-object bound per pawn; exceeding it
fails visibly rather than truncating the graph. Speculative pointer read errors
are explicitly reported in the terminal and stage metadata. Pawn, ownership,
transport, screenshot or required-file failures stop the scan and write
`failure.json`. Probe extents are not verified allocation sizes, so neighboring
data is possible and correlations alone do not establish ownership semantics.

Each phase also saves a **desktop** screenshot with the diagnostic marker.
`selection.png` and `selection.json` preserve the initial operator selection.
`session.json`, `events.jsonl`, stage JSON and the adjacent terminal `.log`
make interrupted sessions reviewable.

After all eleven phases, `candidates.json` compares unsigned 8/16/32-bit,
signed 32-bit, float32 and float64 values. For health it requires both initial
damage levels and repeated damage to move consistently away from full values,
while stamina-only phases remain near full. Stamina uses the reciprocal pattern.
Both increasing deficit fields and decreasing resource fields are considered.
Full/recovered values must agree within 0.2% of the largest observed magnitude,
with a minimum tolerance of 0.00001. Tiny subnormal float decodings are excluded.
Later burst values are included and changing candidates are visibly marked.

Only slots present across all phases with the same address, type and fingerprint
are compared. Missing/changed subjects are listed as excluded, not treated as zero.
`known_raw_checks` records laptop pawn +0x84, +0x4B4 and +0x514 as unsigned
32-bit values. The local control may produce correlations, but its actual HUD
state is not assumed to equal the laptop's phase label.

Candidates remain unverified. No scalar match is inconclusive: health/stamina
may be unreplicated on the desktop, stored indirectly, packed/encoded, or may
have recovered before the F8 read. No runtime offset or substitute reader is
installed by this scan.

To rerun the offline review without another live capture, choose a new result:

```powershell
python scripts/apb_health_stamina_review.py '<completed capture directory>' '<new report.json>'
```

The reviewer verifies saved bodies against their types, fingerprints, lengths
and SHA-256 metadata. Partial scans are preserved but cannot be reported as a
completed eleven-phase review. Existing files are never overwritten.

Implementation validation: Python import/CLI checks and byte compilation passed;
PowerShell syntax parsing passed. Gameplay captures and resource interpretations
require the operator's F8 sequence. No automated tests, fixtures or test
dependencies were added.

The first launch on October 2 stopped before selection because the selected
camera table contained zero typed cameras. Its log/failure.json were preserved.
Startup now waits for an explicit F8 after loading/focusing APB before resolving
the same camera route. This is a readiness gate, not validation that the camera
or laptop resource values are available in the current session.

The first baseline attempt failed with WinError 87 because this scanner asked
EchoDriver for 12288 bytes in one call. The corrected `read_body` splits large
bodies into explicit 4096-byte requests through the same IOCTL, retaining all
requested bytes. Busy-state/release handling was also added for F8. A separate
read-only live transport diagnostic can verify both previously selected instances
without collecting or labelling a health/stamina phase:

```powershell
powershell -NoProfile -File scripts/run_apb_health_stamina_scan.ps1 -OutputDirectory '<new diagnostic directory>' -VerifyReadFrom '<same-process capture directory>\session.json'
```

That diagnostic fails if the saved process, image base or either pawn fingerprint
changed; it does not reselect another pawn. Its `read-verification.json` is
transport evidence only, not proof of health or stamina semantics.

Live read verification on October 2 succeeded for both previously pinned
characters: laptop `0x1F6C97040` and local `0x14858B040`, PID 276 with the
saved creation time. Each full 12288-byte body used three 4096-byte requests;
the diagnostic exited 0. Evidence is saved under
`../drayebr/build-rpm/snapshots/health-stamina-readcheck-20261002-162218`.
The complete F8 phase sequence and rapid-press UI behavior still require
operator gameplay validation.

The eleven-phase run `health-stamina-laptop-20261002-162349` subsequently
completed with exit 0 and three automatic scalar matches. The operator confirmed
PIG less-lethal damage was used for stamina despite the original, incorrect
sprint prompts, and confirmed health fell by approximately 85% (15% remained).
The prompts above and future scanner prompts are corrected; original capture
instructions and candidate results remain intact. See
[APB_HEALTH_STAMINA_FINDINGS.md](APB_HEALTH_STAMINA_FINDINGS.md) for interpretation.
