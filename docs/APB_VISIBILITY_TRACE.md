# F8-confirmed visibility discovery

Rewritten 2026-10-03 to follow the cursor workflow: prepare a specific scene,
press F8, hold until CAPTURE COMPLETE, then prepare the next scene. Waiting for
F8 has no time limit. The sampling timer begins after confirmation; a cue is
never treated as the moment visibility actually changed.

Run from elevated PowerShell in the repository root, with APB in a district:

```powershell
./scripts/run_apb_visibility_trace.ps1 -Hz 15 -Seconds 4
```

Output is a new timestamped session under
`C:/Users/Admin/Documents/source_repos/drayebr/captures/visibility/`, plus its
terminal `.log`. An explicit `-OutputDirectory` requires a new directory with
an existing parent. The launcher requires administrator status, exactly one APB
process, the verified executable build and the existing EchoDriver reader.

## Stationary laptop / moving observer experiment

The completed `20261003-101517-553-in-view` session produced an unverified
pointer/count/capacity lead at pawn +0xD1C/+0xD24/+0xD28. Several other passing
fields were coordinate copies. To remove target movement from the next comparison:

```powershell
./scripts/run_apb_visibility_trace.ps1 -Protocol observer-los -Hz 15 -Seconds 4
```

Place the laptop character near opaque cover where moving the PC around a corner
can expose or hide that same stationary character. Select the exposed laptop with
F8 survey, F5 cycle and F6 confirm. Leave the laptop in that spot for the entire run.
On the PC, move around the cover between captures; keep aiming at the laptop's
location, including when the wall hides it. Turning away/leaving the view is outside
this protocol. Stop moving before F8 for every settled capture. Hold until CAPTURE
COMPLETE. During the explicit observer movement control, walk while keeping the
laptop exposed and its origin in view. F9 discards a mistaken active capture; F11 exits.

There are nine phases: two clear/blocked discovery pairs, one exposed observer
movement control, then two clear/blocked recheck pairs. Rechecks request another
viewing angle. The laptop's position is checked against its selection reference
throughout all captures, with the declared 2-world-unit tolerance. Moving the laptop
invalidates the experiment and fails visibly. Camera identity, ownership, target
membership, world and projection guards remain active. Camera pose may change
between bursts; its baseline resets at each burst. Camera movement within a settled
burst still fails. Only the named observer movement control permits camera motion
during sampling, and that control must observe actual motion.

The entire 16-byte group is saved in each sample as raw hex, pointer, count and
capacity, alongside the original prefix bytes and per-region timing. Its interpretation
is a hypothesis. The recorder never dereferences its pointer. Implausible layouts are
logged and shown rather than accepted. `structure-review.json` reports both raw and
trimmed sample agreement with the predeclared nonzero-pointer hypothesis, retains
every mismatch with its timing and camera pose, and verifies that logged structure
values match the hashed raw bytes. It does not fit a new threshold or accept a product
visibility predicate. Generic candidate filtering remains available as a separate report.

Each completed capture saves its own `canvas-*/structure-review.json` progress
snapshot. The session-level `structure-review.json` is written once, after all phases
finish. Reports are never overwritten; an offline review requires a new output path.

Reproduce the focused offline review with:

```powershell
python -B scripts/apb_visibility_structure.py 'C:/path/to/observer-session' 'C:/path/to/new-structure-review.json'
```

## Operator workflow

Keep the PC character and camera fixed; control the laptop character. Pick a
view containing open space and opaque cover. Keep the witness within that view
throughout the experiment. For the last recheck, choose another opaque wall within that same fixed
view. Avoid shooting, weapon changes, vehicle entry and respawns.

1. Laptop exposed and still: F8 captures the selection survey. Wait for
   SELECTION SURVEY COMPLETE. F5 cycles pawn origins; F6 confirms the independently
   identified laptop witness. A green selection marker does not mean visible.
2. Follow the displayed VISIBLE NOW or HIDE BEHIND WALL cue. Move the
   laptop, settle, then press F8 on the PC when the requested scene is true.
3. Hold that state for four seconds, until CAPTURE COMPLETE. The overlay shows
   remaining capture time and actual rate/gaps. F8 does not stop a burst.
4. During the movement control, keep walking through open space while remaining
   fully exposed. The capture requires observed target displacement.
5. F9 discards a mistaken active capture and returns to an indefinite F8 wait
   for the same phase. Discarded bytes and the reason remain saved. F11 exits;
   unfinished captures remain partial evidence.

The 12 phases are clear / wall-blocked / clear / wall-blocked / clear; a
moving-but-clear control; then three clear / wall-blocked recheck pairs. Rechecks
request different exposed spots and a different opaque wall, to challenge
position-based associations. Scene labels are explicit operator observations.

Wall-blocked means fully hidden by an opaque wall while the pawn origin remains
inside the PC view. Leaving the view is outside this experiment. Origin projection
checks do not prove that the whole body is visible or hidden. Do not press F8 for
partial exposure; F9 rejects a mistaken capture. Ownership checks remain strict.

On 2026-10-03 the leave-view step failed with "On-foot character has no reciprocal
current position component". Five completed in-view captures remain preserved in
`drayebr/captures/visibility/20261003-095703-184`. The failure does not establish
why the component link disappeared. The leave-view phases have been removed at
the user's request; the recorder starts a fresh selection/session after changes.

## Scope and integrity

The recorder selects pawns from the existing native roster traversal: typed
World +0x48C, then each typed character/vehicle node's +0x350 link. Every node's
type, instance, World backlink and next edge are rechecked, together with the
World/head at both ends; invalid, cyclic, changed or partial lists are rejected.
The bound is 512 nodes, matching `src/apb_grove.rs`. This is the declared
selection source, with no automatic module-window substitution. Ownership checks
and local-player exclusion still apply. The module survey remains evidence for
scope bounds; rooted roster headers augment that census in a separate saved file.
Startup reports live roster counts, and selection saves `roster-0001.json`,
`roster-0001.headers.bin` and `roster-0001.bodies.bin`.

The selected pawn prefix, its currently registered component prefixes and the
typed world prefix are sampled. Requested extents are 0x3000 bytes for the pawn
and 0x1000 for components/world, clipped before independently registered neighboring
objects found in the selection census. These are probes, not verified allocation
sizes; unknown neighbors may remain. Registered components are not asserted to be
skeletal meshes. No full writable-memory scan or new pointer hunt is substituted.

Every round checks the rooted camera, local possession, world, selected pawn
fingerprint, controller/attachment and component membership, plus each sampled
instance fingerprint. Memory reads use EchoDriver exclusively. Foreground loss,
read failure, replaced ownership, inconsistent projection and observer camera
movement fail visibly, with failure.txt, session.json and the full terminal log.
Changed witness position during a settled burst discards that burst explicitly.

Camera lookup verifies one exact typed camera in the first 128 table slots, then
rechecks its root, slot, instance fingerprint and camera shape. A changed table
entry outside that binding no longer invalidates the whole lookup by itself. Its
new pointer is classified and its slot rechecked; a new camera-marker entry,
unreadable classification or another change during classification rejects the
lookup visibly. Root, selected-slot and camera-instance changes remain fatal.
No alternate root or reader is substituted, and no camera is silently reacquired.

`camera-table-events.jsonl` records accepted non-camera changes and rejected
lookups with exact changed slot indices, old/new pointers, available class markers,
before/after raw table bytes, root pointers, selected-camera fingerprints and
monotonic timing. The terminal log also reports each event. Startup performs a
live camera-binding check before waiting for F8. Slot relocation still requires
fresh selection in this fixed-identity recorder.

Camera drift is measured against the first burst: default position tolerance
1 APB world unit and maximum matrix-element change 0.001. The witness displacement
limit within settled captures is 2 world units. These are declared capture guards,
not established visibility tolerances. Launcher options expose all three values;
there is no automatic tolerance relaxation.

Reads are sequential and non-atomic. Each region has its own start/end time on a
single session-wide monotonic clock. Each capture includes before/after PNGs,
raw.bin, samples.jsonl, capture-start.json, and a capture.json with file hashes,
achieved rate, missed deadlines and maximum gap. Before/after images cannot prove
all intermediate scene states. Screenshots use the existing client-image path;
the UI uses the existing layered-window style.

## Candidate filtering

`apb_visibility_candidates.py` validates byte ranges, hashes, fingerprints,
sequence and fixed layouts. Discarded/unfinished captures are listed and excluded.
The first and last 0.5 seconds of each burst are excluded from fitting. Discovery
captures alone select candidate rules; neither the movement control nor rechecks
select thresholds or recalibrate clocks. All complete bytes remain available.

- **Bits:** individual bits that remain constant across repeated discovery clear
  captures and take the opposite value in every wall-blocked capture.
- **Scalars:** aligned finite float32 values with separated discovery ranges.
  A fixed midpoint threshold is checked against controls and rechecks.
- **Clock leads:** nonnegative world float32 fields that follow session elapsed
  time with slope 0.9..1.1 and maximum residual 0.25 seconds. Fit coefficients are
  kept fixed for later comparisons. This does not establish TimeSeconds identity.
- **Timestamp pairs:** candidate world-clock minus pawn/component float32,
  corrected for measured sequential-read skew using the discovery clock slope.
  Discovery timestamp values must not decrease beyond 0.05 seconds, clear ages
  must be at most one second, and all discovery ages must lie in -0.25..3600 seconds.
  These explicit search filters implement the proposed recency hypothesis; they
  are not universal engine guarantees. Only in-view clear versus wall-blocked fits are evaluated.

Known pawn XYZ and immutable header identity bytes are excluded. Individual bits
and aligned float32 are the supported search encodings; integer enums, float64,
unaligned fields and query results that are not stored are not comprehensively
searched. A candidate comparison pass requires at least 95% correctness in every
relevant control/recheck capture, and both clear and the corresponding negative
state must be present in rechecks. Timestamp pairs additionally require their
clock fit to remain valid. These are exploratory associations, not LOS validation.

Progress counts are saved at selected phase boundaries. discovery-candidates.json
records the discovery fit before controls; candidates.json contains all candidate
rules and per-capture comparisons, including failures. Zero survivors is reported
directly. No candidate enables Visible only, aim, trigger, bones or product LOS.

For a reproducible offline review, use a new report path:

```powershell
python -B scripts/apb_visibility_candidates.py 'C:/path/to/v2-session' 'C:/path/to/new-review.json'
```

## Cross-mission float64 review (October 3)

Reviewing saved captures by pawn-relative offset instead of absolute address found
a strong render-recency candidate that the aligned float32 analyzer missed:

| Owner | Offset | Encoding | Observed role |
| --- | --- | --- | --- |
| Pawn | `+0x2E0` | little-endian uint64 pointer | References the recorded World in all 2,349 complete raw samples |
| Pawn | `+0xB4` | little-endian float64 | Timestamp advances during clear phases and freezes during blocked phases in both main runs |
| World | `+0x6A0` | little-endian float64 | Clock advances approximately one second per second in all nine sessions |

Use each current pawn's World reference, rather than retaining an address from a
previous mission. The proposed comparison is `WorldClock - PawnTimestamp <= 1.0`;
the offline review additionally checks finite/nonnegative timestamps and allows
0.25 seconds of negative age for sequential reads. Clock slope is fitted using
discovery samples only to correct measured read-time skew. The original one-second
bound is fixed, not fitted to labels.

Across four pawn instances, nine sessions and 40 completed phases, this comparison
agrees with 2,286/2,349 raw samples and 1,683/1,727 trimmed samples. In the main
target-motion and stationary-target observer runs, all 879 trimmed samples across
20 phases agree. Three initial blocked samples in the target-motion run have
ages below one second. An earlier observer blocked capture
(`20261003-104918-694-observer-los/canvas-0002`) updates the timestamp throughout
and disagrees on all 60 raw / 44 trimmed samples. That exception remains unresolved.
The endpoint screenshot alone does not establish its cause.

An every-byte-offset float32/float64 temporal scan of pawn and component prefixes
finds only pawn `+0xB4` satisfying the advancing-clear/frozen-blocked pattern in
both main sessions, including movement controls. These are retrospective pattern
findings within one executable build and launch; the native property name and
producer are unverified. Render recency can lag hiding and is not instantaneous
geometric LOS. The existing generic live candidate reader supports four-byte
scalars; use the separate explicit float64 reader for this candidate.

Reproduce from the saved v2 evidence using a new output directory:

```powershell
python -B scripts/apb_visibility_recency_review.py 'C:/Users/Admin/Documents/source_repos/drayebr/captures/visibility' 'C:/path/to/new-recency-review'
```

The report validates source hashes, exact owner types and pawn-to-World references,
records all complete-phase results and explicitly lists excluded incomplete or
discarded captures. Original evidence and prior reports remain intact.

For live checking, run the following from elevated PowerShell with APB in a district:

```powershell
./scripts/run_apb_visibility_recency_live.ps1 -EvidenceReport 'C:/Users/Admin/Documents/source_repos/drayebr/captures/visibility/cross-session-recency-20261003-112629-297/recency-evidence.json'
```

F8 surveys the current World pawn list; F5 cycles witness origins and F6 confirms
the laptop. F8 starts live reading. F1 labels exposed, F2 hidden, F4 partial and
F3 unknown. Labels persist until changed; candidate predictions update independently.
F8 saves an image and F11 completes the recording. Each label change also saves
an image. Both characters may move. The exact same typed camera may move between
table bindings after revalidation, with every relocation logged. Camera instance
or ownership changes fail visibly rather than automatically selecting another pawn.

World clock reads bracket each pawn timestamp read; interpolation supplies its
comparison time and both raw age bounds are retained. The overlay shows RECENT
or STALE, age and raw decoded clocks. All samples remain saved. Agreement summaries
include both all labeled samples and samples at least 1.5 seconds after a label
change; unknown/partial labels do not contribute to clear/blocked agreement.
Only the reviewed executable hash is accepted, with fresh explicit witness selection.
This live observation run is needed before claiming live candidate confirmation.

## Live raw recheck

Choose an explicit candidate ID from candidates.json and keep the same APB launch:

```powershell
./scripts/run_apb_visibility_live.ps1 -SourceDirectory 'C:/path/to/v2-session' -CandidateId 'timestamps-1'
```

The reader verifies the recorded process, witness, observer/camera, world and
component instances. It displays raw values and records candidate predictions
without declaring them engine visibility. PC camera movement is permitted in this
separate live diagnostic. F1 labels clear, F2 opaque-wall blocked, F4 partial; labels start unknown. F8 saves an image; F11 ends the live session.
The output has samples.jsonl and a session manifest, or retained partial evidence
and an explicit failure. A restart requires new discovery: no stable cross-launch
pointer path to a visibility field has been established.

## Legacy evidence and validation

Older v1 segment recordings remain untouched. Review those with
`apb_visibility_trace_review.py`; its format does not describe this v2 protocol.
The October 2 scoped review found no reliable clear/blocked field in its sampled
prefixes. More repeated scenes cannot establish native property identity or prove
that fields outside this scope are absent.

Syntax/import/help checks (no new automated tests):

```powershell
python -B -c "import ast,pathlib; [ast.parse(p.read_text(encoding='utf-8-sig')) for p in pathlib.Path('scripts').glob('apb_visibility_*.py')]"
python -B scripts/apb_visibility_trace.py --help
python -B scripts/apb_visibility_candidates.py --help
python -B scripts/apb_visibility_live.py --help
```

These checks do not establish live capture or visibility correctness. A live
session requires the operator's scene preparation and witness selection.
