# Memory coordinate source status

Current native constants and executable gate were updated on October 18;
see [core update and remaining validation](APB_CORE_UPDATE_20261018.md).
Earlier dated sections below retain their historical addresses and evidence.
The October 18 moving-player check rejected render recency as wall visibility:
an obstructed moving player can remain recent indefinitely, not only briefly
after hiding. The existing filter is still recency, not confirmed obstruction.

The EchoDriver repository (`..\drayebr`) now has a read-only process-memory
IOCTL and an administrator-only diagnostic client. `src/atlas_reader.rs` is the
matching Verux transport. Neither component writes to APB; Verux mouse movement
remains on the existing Arduino serial path.

## Native integration: October 1, 2026

### Render-recency visibility: October 3, 2026

Memory aim now has **Visible Only** under Target Conditions, enabled by default
and saved as `prism_only`. Older profiles without this key receive the
enabled default. The control is disabled for Computer Vision. ESP and radar keep
their current player filters; their display does not become visibility-filtered.
Memory trigger continues to use its independent cursor detector.

`src/apb_prism.rs` reads pawn `+0xB4` as a little-endian float64, with a World
clock at `+0x6A0`. The pawn's `+0x2E0` reference must match the verified current
World. Exact types, fingerprints and references are checked around each read;
nonfinite/negative clocks, backward clocks and timestamps more than 0.25 seconds
ahead of the clock withhold acquisition and log the reason. A zero timestamp is
treated as unrendered. The tested comparison is age <= 1.0 seconds, so it can
remain eligible briefly after hiding; it is render recency, not a ray trace.

The World clock brackets the timestamp read, and the later clock is used
conservatively. No host timer division is involved. At frame completion all ages
are checked again against a current verified World clock. Stale pawns are excluded
from aim selection; an unavailable sample withholds the frame through the existing
logged reacquisition/error path. No alternative detector is substituted. Loss of
an eligible memory target clears queued movement and decay. Frame diagnostics
include stale-render counts and whether Visible Only is active.

Camera lookup now checks unique typed camera candidates across table snapshots
instead of requiring byte-for-byte equality of all 128 dynamic object slots.
Non-camera insertions/removals can change the table while the camera remains valid.
Only the exact same address/fingerprint can retain tracking after rebinding;
actual instance changes still use the existing stable-camera acquisition path.
The float64 diagnostic recorder likewise allows and logs relocation of the same
verified camera. Strict discovery experiments retain their fixed binding policy.

Reproducible validation commands (no new automated tests):

```powershell
cargo check --bin verux_apb
cargo build --release --bin verux_apb
cargo build --example apb_memory_probe
# Elevated, while APB is in a district; use a new report path:
./target/debug/examples/apb_memory_probe.exe --visibility-report C:/path/to/new-native-visibility.json
```

The native probe enforces the executable hash and rooted roster, uses the same
production float64 sampler, and saves current per-pawn clock/timestamp/age/results.
It sends no input. This verifies native reading, not scene ground truth; the live
operator check supplies scene labels independently.

Source selection now lives in Aimbot and Triggerbot. The conditional Memory tab
contains EchoDriver and APB refresh buttons. ESP and all drawing controls are in
the always available Visuals tab. Enabling ESP opens the memory pawn source even
when both input features use Computer Vision. APB refresh
verifies the process and logs its PID; there is no persistent PID/status display.

`apb_atlas` now uses the verified camera pointer table at module RVA `0x3DBF110`
(first 128 slots), exact camera type `(0x2B5EEA8, 16920)`, matrix `+0x680` and
position `+0x6C0`. A completed initial roster discovery already validates its
camera at both ends. A genuinely new camera found during frame reacquisition
requires two stable resolutions at least 500 ms apart. Table slot relocation
does not change object identity: the exact address/fingerprint is relocated in
one table read and rechecked before retaining tracking. The roster worker audits
table uniqueness every two seconds, rather than scheduling a full lookup in
the frame loop. Normal refreshes validate the known camera binding at both ends;
each frame verifies its binding, typed instance and live matrix. Camera-relative
row-vector projection uses the live
APB client size and screen origin.

`apb_orchid` ports the successful diagnostic ownership checks: exact character
pawn `(0x2AAEFA0, 5778)`, canonical XYZ `+0x88`, reciprocal canonical collision
ownership, and camera/world/controller local ownership.
Pawn `+0x248` must reach exact collision type `(0x3197448, 3269)` with
both `+0x44` and `+0x7C` pointing back to that pawn. The slot, component instance
and backlinks are rechecked. The registration array `+0x140` is no longer an
ownership gate; vehicle transitions can change its membership independently
of the canonical character. There is no alternate proof if the selected
collision ownership checks fail. The same character proof applies on foot and
while attached to vehicles, without depending on a vehicle attachment pointee
that may be changing during exit. The attachment field itself is rechecked
along with character instance, controller and World. Local possession still
uses its existing reciprocal controller/character/typed-vehicle checks.
`apb_grove` now follows World `+0x48C` and pawn `+0x350`, replacing the recurring
module-window surveys. It accepts the captured character and vehicle types,
traverses vehicles without selecting them as characters, caps traversal at 512
nodes, rejects cycles, and rechecks every node's fingerprint, World and next link.
World identity and head are rechecked before publishing a complete roster.
Unsupported types and read failures are logged with the relevant address.

The worker keeps a 500 ms full refresh while APB is foreground. Between full
reads, it checks the World fingerprint/head and up to eight saved node links
every 10 ms in a bounded round-robin. Changed head, fingerprint, World or next
link requests a complete refresh immediately. The monitor only schedules the
same guarded traversal; it never publishes its partial observations or cached
coordinates. A head insertion can therefore be noticed on the next poll;
interior changes require their node's turn. Read cost is bounded to nine reads
per monitor poll rather than a full district traversal every 10 ms. World
changes request an immediate refresh. Changed lists are retried after
50 ms without publishing a partial traversal. Read errors are logged on change
and retried through the same implementation; errors persisting for 30 seconds
use existing source-stop reporting. A roster older than two seconds cannot
supply targets, including if it expires while a frame is being sampled. The
runner drains pending updates and applies the newest complete snapshot.
Numeric offsets remain tied to the verified executable hash. Saved-capture
replay agrees with six captured character subsets; complete mixed-list coverage
and runtime performance across live transitions remain unverified.

### Independent feature acquisition: October 3, 2026

`ProfileConfig::runtime_requirements` provides one enabled-feature plan for pawn
memory, cursor memory, capture, overlay and serial input. Disabled aim/trigger
sources do not activate their backends. Radar/ESP alone do not start the Arduino
sender. Startup logs record the plan and each feature's player filter.

Memory frames request the union of three independent player filters: aim, ESP and
radar. Aim requests its selected Target Players filter even with both displays
off, and requires body bounds independently of ESP Boxes/Dots. Visible Only is
sampled only for aim candidates, so an ESP-only pawn cannot withhold aim through
a visibility sample. Classification and selection use the same filter semantics;
unknown mission roles remain excluded from aim. Enabling ESP cannot broaden the
aim filter or provide an otherwise missing aim prerequisite.

| Enabled feature/source | Data requested |
| --- | --- |
| Memory aim | Selected aim player filter, verified body bounds, optional render recency/distance |
| Memory trigger | Independent verified cursor reader; no pawn roster or DXGI requirement |
| Computer Vision aim/trigger | DXGI capture for that enabled feature |
| ESP | Its own player filter and display style; boxes request bounds |
| Radar | Its own player filter and verified local/remote positions |
| Predictive braking with aim | DXGI capture in addition to the selected aim source |

Combined configurations activate the union of these requirements. Capture and
cursor failures use the existing explicit error path; no source substitution is
introduced. Computer Vision aim visuals retain desktop coordinates when an
independent ESP/radar frame is unavailable, while memory markers remain withheld.

Mission token `+0x990` and side `+0x994` determine teammate/opposition/outside
roles using the explicitly observed active and inactive states. No faction,
distance or component count substitutes for membership. The user confirmed the
subsequent live classifier worked. The native aim source uses its selected Target
Players filter; changing projection display filtering does not broaden aim eligibility.
Unknown mission states are logged, excluded from aim, and colored amber only
when All Players is displayed. Missing local pawns temporarily leave remote
roles unknown; changed local context invalidates the frame. Stale remote pawns
are withheld, never projected from cached coordinates. Collision bounds are
read for selected Memory aim roles and any role requested by ESP Boxes. ESP Dots
alone do not request collision bounds. Classification precedes component/position
reads: a role is acquired only when aim, ESP or radar requests it. The
position sampler also verifies each pawn's World against the local World.
Memory target loss cancels queued movement and decay tails.

Acquisition diagnostics are written to `verux_apb.log`, with no new status panel.
Every five seconds, a frame summary reports roster age, role counts, filtering,
ownership/World/projection/bounds rejections, projected and eligible counts, and
frame duration. Roster logs include duration and character additions/removals.
Per-pawn failure suppression uses the instance fingerprint as well as address,
so a reused address does not suppress a new instance's error.

ESP extends the existing GDI `AuroraWindow` visuals renderer; there is
no additional rendering backend. It follows the APB client rectangle and hides
when the source has no current frame or APB loses focus. Colors, projection
filter, enable switch and configurable toggle key persist in existing profiles.
The default is ESP off, Boxes, opponents-only, with F10 as the toggle key.

### Visual controls, distance and target priority

Visuals contains an Aim Visuals switch/keybind and independent FOV Box, Aim
Target Box and Aim Line switches/colors. The target box no longer depends on
the FOV switch. Aim Line Origin offers Center and Top (top center of the APB
client viewport for memory visuals). One line ends at the selected target's
box center plus the horizontal aim shift. ESP has its own switch/keybind,
Boxes/Dots dropdown, mission filters and role colors. Memory aim boxes and
ESP boxes use the same current collision bounds and outline renderer. A
missing/invalid bound withholds the box and logs why; it never substitutes a
dot. Dots remain available by explicit selection.

Display Distance labels ESP and the selected Memory aim target using
`sqrt((remote_x-local_x)^2 + (remote_y-local_y)^2 + (remote_z-local_z)^2)`.
The origin is the character with verified local possession, not the camera.
Both positions must belong to the same current World. Labels show meters,
using the user's observed calibration `100 world units = 1 meter` (8000 units
becomes 80.0 m). `APB_UNITS_PER_METER` holds this APB-specific scale; target
ranking keeps raw world distance, so the label conversion does not change
target order. Local XYZ is sampled once
per frame only when distance labels or Closest Distance need it. Local pawn
identity/World and possession are rechecked. Missing local position logs the
reason, withholds distance labels and prevents Closest Distance selection.

The Aimbot Target Priority dropdown offers Closest Aim (squared screen
distance from the reticle to the projected body center) and Closest Distance
(3D character separation, with Closest Aim breaking ties). Both retain
opponent classification, live ownership/bounds and grab-region intersection
requirements. Closest Distance requires the Memory aimbot source; selecting
it with Computer Vision produces an actionable logged source error rather
than substituting another ranking. Computer Vision Closest Aim ranks name
box centers by screen distance and keeps its horizontal-only movement.
An actor origin outside the viewport no longer rejects an otherwise visible
verified body; fully off-screen bodies remain excluded.

The Block key now gates movement and shooting only. Roster refresh, frame
collection, active cursor observation, activation key state and rendering
continue. The serial sender independently checks Block and clears queued
movement/shots, so a slow collection frame cannot postpone input blocking.
Both visual toggle keys respond once per press.

Existing `lanterns`, `lantern_key` and related profile keys are
retained for compatibility under the ESP UI name. New keys are `aurora_style`,
`show_distance`, `orbit_order`, `aim_line_origin`, `aim_target_box` and
`aim_target_color`. Old profiles inherit the target-box switch/color from
their former shared FOV settings; subsequent saves persist them separately.
New styles/origin/priority default to Boxes, Center and Closest Aim.

Reproducible validation uses `cargo check --offline`,
`cargo test --offline --lib`, the saved-capture command below, and
`cargo build --offline --release --bin verux_apb`. Live gameplay is required
to validate slot relocation, block behavior and actual drawing. Startup logs
identify memory pipeline revision `2026-10-01 vehicle-exit`; logs saying "Memory pawn survey
refreshed" identify an older executable.

### Memory Y correction and captured collision bounds

The user's rule is: begin Y movement only outside the body, enter at least
25% inward from the nearest upper/lower boundary, then release. A latched
correction continues across the outer edge until the central 50% band is
reached. Starting inside the body does not initiate vertical correction.
Switching pawn instance, target loss, blocking or releasing aim resets the
correction. Computer Vision remains horizontal-only. Memory Y movement uses
the existing ramp, proximity and smoothing pipeline and Arduino Y packets;
queued corrections are replaced and cleared when the body band is reached.
Predictive braking continues to control X; Y uses the existing selected
smoothing curve. No fixed height or NPC-derived size is substituted.

The saved `mission-labels-20260930-2/collision_bounds_review.json` records 14
component snapshots. Pawn `+0x248` reaches exact type `(0x3197448, 3269)`;
component `+0x44` and `+0x7C` both link back to the pawn in all 14. Component
`+0x108` contains origin, XYZ box extents and sphere radius. The radius equals
the extent-vector length in all samples. `+0x234/+0x238` half-height/radius
match the extents in all samples: standing half-height 90/91 and radius 33/34,
with one half-height 60 sample accompanied by pawn eye-height 48 and lower
pawn Z. Bounds origin exactly equals the captured pawn XYZ in 13/14; the
remaining sample differs by 0.586 in X and 0.031 in Y across sequential reads.
This is collision-component evidence, not verified visible mesh/bone scale.
Reinspection confirmed the exact owner links in all 14 raw annotations.
Annotations 0008, 0009 and 0011 have zero registered components while retaining
both collision owner links; those three characters are attached to vehicles.
The evidence validates the collision ownership layout, but does not measure
the first frame after an exit. A live read-only probe on October 1 was denied
EchoDevice access (`0x80070005`); the new exit latency still requires an
administrator-launched gameplay check. Ownership recovery is logged when a
previously rejected pawn starts supplying current coordinates again.

Eight live world-bound corners are projected without clipping off-screen
edges. Pointer, exact type, instance fingerprint, reciprocal ownership and
stance dimensions are rechecked. A pawn without valid current bounds is
excluded from Memory aim and logged; its independently validated point can
still be displayed when ESP Dots is explicitly selected. Target eligibility uses body-box intersection on
both axes, so a body entering the grab region is not discarded solely because
its center is outside. No point-only vertical fallback is used.

Online research did not supply a verified player height from APBDB. The local
`APBGame.ini` value `m_fPedestrianHeight=180.0` belongs to NPC pedestrians.
The developer's [character scaling notes](https://apbreloaded.blogspot.com/2014/)
describe nonuniform scaling, and [crouch hitbox notes](https://apbreloaded.blogspot.com/2013/01/)
describe different standing/crouching collision behavior. These are context,
not native offsets or current numeric player dimensions.

The native integration compiles, but the executable's new controls, collision
bounds and Y correction still require live acceptance. Check stand/crouch,
near/far, vehicles, crossing above/below the body, already-inside-body behavior,
mission end/new opponents and target loss. The user elected to validate later.
Occlusion and health/death fields remain unverified.

Null district/world ownership and camera transitions withhold output and log
reacquisition through the same memory implementation. Unexpected inconsistent
local/camera data persisting for 30 seconds stops with an actionable error.
Transport/process failures stop visibly. Computer Vision is never activated
automatically when Memory is unavailable. Memory triggerbot polls its verified
cursor path on a separate two-millisecond worker, so pawn projection and survey
reads cannot delay the trigger key/cursor check. Pending shots are cleared on
key release, block, lost focus, unselected cursor state, and shutdown.

For future interactive APB memory and pointer scans, let the player prepare each
state without a timer and start each capture with an in-game key press. Keep the
terminal readout visible for raw-value confirmation. The workflow is recorded in
`docs/APB_CURSOR_MEMORY.md` under "Interaction preference for future scans."

## Current validation limits

- Exact executable hash and current module base are verified before opening
  the source. Supplied absolute heap addresses are never configuration roots.
- User-validated diagnostic coverage includes district changes, restart,
  distance, vehicles, mission classification and death/respawn reacquisition.
- The native app still needs a live acceptance run; compilation is not gameplay
  or visual acceptance. Collision bounds and Y behavior need live visual checks;
  visible mesh scale remains unverified.
- No runtime source fallback or new automated tests were introduced.

`drayebr\build-rpm\EchoClient.exe --read <pid> <hex-address> <byte-count>`
can capture small address/value traces once the newly built driver is loaded.
The running EchoDriver service was updated to the read-capable build during the
live probe. A pointer walk or projection cannot be verified when the game is not
running.

## Live read on 2026-09-28

APB.exe was running and the read-only EchoDriver IOCTL returned the `MZ` header
at `0x140000000`, matching the executable's preferred base. The executable's
product version is `1.33.0.859279` and its
SHA-256 was `3BECA02821F50A7B8FEF3E16923FA3FCA0D0B0DD49CC9622520EA500001E1640`.
The supplied `GNames` address held a readable user-mode pointer, but the first
eight bytes at the supplied `GObjects`, `GEngine`, and `UWorld` addresses were
`0x138295EC36396037`, `0x271A9E096E26E32E`, and `0x5232EB441B0E9663`.
Those values are not usable user-mode virtual addresses. The executable also
contains a `.themida` section. These observations do not establish whether the
three globals are encoded, have different layouts, or are stale offsets. No
pointer was dereferenced or target selected from them.

The current implementation uses the later verified table traversal and view
projection described below, restricted to this executable hash.

The additional older offset set was probed in the same running process. Its
`GObjects` location starts with a null entry followed by readable object-like
addresses, but that alone does not identify live players or cameras. The
objects reached from indices 18, 5776, and 6714 did not have matching values
at the supplied `Object_ID = 0x2C` field. The older `GNames`, `GEngine`, and
`UWorld` locations did not yield a usable plain pointer chain either. The
older pawn and camera field offsets therefore remain unverified; mixing them
with the newer globals would not produce a reliable coordinate source.

## June 2026 notes checked against the live build

The user-provided notes describe product version `1.32.0.853180`, so their
offsets are hypotheses for a different build. Their proposed structural scan
and UObject field validation are useful diagnostic methods, but their automatic
fallback chain must not be adopted: a failed Memory source must remain visible
and logged. Their internal DLL, swapchain hook, and write-related material are
outside the requested RPM/Arduino path.

Read-only probes of the notes' `GObjects` and `GNames` RVAs yielded nonpointer
values in the current process. The notes' `UWorld` RVA contains a series of
readable-looking addresses, not a confirmed world object pointer. No field
from those RVAs has been accepted as a current-build offset.

The older `GObjects` candidate at `0x143d312f0` was scanned over 32,768
8-byte slots: 7,411 had plausible addresses, 7,392 object headers were
readable, and only 20 had a `+0x20` value of `18`. None of the sampled headers
had `5776`, `6714`, or `16845` at `+0x20` or `+0x2C`. This does not validate
the notes' object layout or identify a local pawn, camera, HUD, or view matrix.
A bounded scan using the notes' `UWorld + 0x3AC` and `ULevel + 0xA0` pattern
produced one superficial match. Its alleged local-player field was
`0xFFFFFFFF00000000`; three of six alleged actors were null, one was
unreadable, and one pointed to UTF-16 text rather than an object. The match
was rejected, so this pattern does not establish a world pointer on the
current build.

With the user in a live district on the same PID, a direct scan of the
7,392 readable object headers found no `+0xA0` TArray with at least eight
entries and four actor pointers whose vtables lie in the APB image. This
rules out using that specific June 2026 actor-array pattern on the scanned
table; it does not rule out a different current-build layout.

The supplied `GNames` address (`0x143cfac90`) is also the start of a long
run of readable object-like pointers in this process: 28,571 of the first
32,768 slots yielded readable headers. Its apparent `+0xA0` array candidates
failed actor validation: their entries had invalid or implausible position,
health, and object fields. Thus the table's role and current actor layout
remain unidentified. A controlled still/turn/move snapshot comparison of
the older table likewise produced no stable pawn-position lead.

## Controlled live-district captures

Three read-only snapshots were taken with the user stationary, after a
camera-only left turn, and after a forward movement. The larger pointer table
contained 24,413 object addresses common to all three. Two class-matched
objects had plausible `+0x88` FVector positions that stayed fixed during the
camera turn and changed during forward movement. A timed jump-in-place trace
then identified the local-position object for this session: its height rose
from about `993.65` to `1152.33` and returned, while its X/Y stayed nearly
fixed. The other candidate stayed at height `445` during that trace.

This confirms `+0x88` as a position field for one live local object in this
build. It does **not** establish a repeatable lookup after restart. Both
objects shared the `+0x3C` class marker `0x1285`; the supplied class/object IDs
were not found at that field in the sampled table. The two supplied table
addresses overlap: `0x143d312f0` is 27,852 pointer slots after
`0x143cfac90`. Their original `GObjects`/`GNames` labels therefore cannot be
used as validated global definitions. A scan of the current executable's
writable data found references to the local object only in pointer-table slots,
not a direct static global that identifies the local player.

The supplied camera positions at `+0x480` and yaw at `+0x490` did not follow
the camera-only turn in the sampled object table. A current camera candidate
and projection were subsequently found below, but no local-controller pointer
walk has been validated. Absolute object addresses and table slots from these
captures are transient and must not be hardcoded into the product.

## Repeated pitch-extreme probe

The user held the camera fully up, fully down, up again, and down again while
remaining still. Four additional read-only snapshots were captured. A unique
sampled object with class marker `16920` had camera-like data: a vertical
direction component near `+1` at `+0x6AC` when looking up and near `-1` when
looking down; an FVector at `+0x6C0` stayed close to the verified local-player
position and changed its height from about `916.5` to `1307.8` with pitch.
Its 4-by-4 float block at `+0x680` also changed coherently between the two
extremes. The same values repeated on the second up/down cycle.

Using the `+0x680` block as a row-vector view/projection candidate, after
subtracting the `+0x6C0` camera position, placed six sampled player-class
positions within approximately one normalized viewport in an earlier
near-level snapshot. The column-vector interpretation gave inconsistent depth
for nearby objects. The candidate was then checked against simultaneous screen
pixels as described below. Its lookup has not been reproduced after a game
restart.

## Simultaneous screen and memory check

A foreground APB client capture and read-only memory sample were taken within
roughly 56 ms of each other at a 1280-by-1015 client size. The frame showed a
visible purple player near the center of the screen. The sampled class-matched
position object at `0x2300A6000` projected to approximately `(684, 444)`
using row-vector multiplication of `(world position - camera position, 1)` by
the `+0x680` matrix. Its torso was visually near `(680, 445)` in the saved
frame. This validates the candidate projection against one live point in one
frame. The column-vector convention had already produced implausible depths
for nearby objects.

The synchronized evidence is under `..\drayebr\build-rpm\snapshots` as
`apb-18292-projection-20260928-200555.png` and the matching `.json`. The
addresses are session-specific. The later restart check below verified the
camera and local-player lookup once across a new process. The current Memory
source targets validated nonlocal player positions as points. It does not yet
classify opponents or estimate body dimensions.

## Read-only discovery diagnostic

`scripts/apb_discover_readonly.py` now uses the process PEB to locate the APB
module base, verifies the executable's SHA-256, scans the observed table RVA
`0x3CFAC90`, and requires exactly one camera-class object (`+0x3C = 16920`).
It checks class-4159 controllers with matching pointers at `+0x140` and
`+0x3C0` to a class-4741 position object. The later restart check added a
class-6683 local link at `+0x148` to distinguish the local controller when
another player has the same position relationship. The camera matrix and
position must be finite and close to the local position. Failed or ambiguous
checks terminate with an error; there is no guessed address or alternate path.

The administrator-only wrapper `scripts/run_apb_discovery.ps1` ran this scan
against the still-running PID 18292. It found camera `0x36FC6B00`, controller
`0x17169A3E0`, and local position `0x16C1C5000`, matching the independent
controlled captures. The scan took about six seconds and found 28,469 readable
headers with one failed header read. Its JSON and status log are saved in
`..\drayebr\build-rpm\snapshots`.

The initial scan verified rediscovery within the same APB process. The restart
validation below verified a revised local discriminator across two launches.
Combat eligibility was left for later; the current source includes validated
nonlocal player-linked positions regardless of faction.

The diagnostic also checks class-4155 owner objects for validated class-4741
position links at `+0xA0` or `+0x1E0` and reports their normalized screen
coordinates as **unclassified** actors. A later scan in the same social-district
session found one other linked position and the local position, with no owner
read errors. The visible player in the earlier screenshot was a random social
district player. That frame cannot validate teammate, opponent, or dead-player
filters; those checks need separately labeled combat-district evidence.

`src/atlas_lantern.rs` contains the checked row-vector projection for
these camera samples. It returns APB client-relative pixels, rejects invalid
or nonfinite data, and reports points behind or outside the view separately.
`src/apb_atlas.rs` uses it to produce the runner's centered player point.

## Restart validation in social district

APB was fully restarted and came back as PID 8244. The executable hash and
module base were unchanged, but the camera and position object addresses were
different. The same table RVA still contained one camera-class object. Two
class-4159 controllers each had matching `+0x140` and `+0x3C0` links to a
class-4741 position. A first diagnostic correctly stopped on that ambiguity.
Camera distance was 261.5 units to one candidate and 816.3 to the other, but
distance alone became ambiguous when the user walked closer to the other
player: 268.5 versus 440.6 units. The near candidate moved about 400 world
units with the user while the other remained at `(30987, 33653, 1239)`.

Comparing controller bytes across both process launches found an additional
`+0x148` link in the independently confirmed local controller. Its pointee is
a table object with class marker 6683. The nearby player controller has a null
link there. The diagnostic now requires exactly one controller with that
class-6683 link and a position within 400 units of the camera; it fails if
missing or ambiguous. With those checks it rediscovered camera `0xFC8A6080`,
controller `0x103B36660`, and local position `0x103916400` in PID 8244.

The first process exposed other positions through class-4155 owner links at
`+0xA0` or `+0x1E0`. The restarted process did not expose those links, although
the second class-4159 controller did link to another player position. The
native source explicitly scans both validated relationships and deduplicates
the positions. It produces a point target rather than an estimated body box.
Combat filtering and body dimensions remain future work.

## Native Memory source verification

The Rust source checks the APB executable SHA-256, resolves its module base
through the PEB, scans the table, and requires a unique camera and local
class-6683 controller link. The table holds the controller objects, but a
controller's position pointee can temporarily disappear from the table while
remaining valid and readable. The native source validates the pointee's class
and position directly. A read-only native probe then rediscovered PID 8244,
camera `0xFC8A6080`, local position `0x103916400`, and one other player-linked
position `0xFC4B4800`. With APB foreground, it returned the centered point
`(63, 137)` in the 1280-by-1015 client. The matching client capture is
`..\drayebr\build-rpm\snapshots\apb-8244-native-view-20260928.png`; the
point at approximately `(703, 644)` falls on a visible player's body.

`cargo check --offline`, `cargo build --offline --example apb_memory_probe`,
and `cargo build --offline --bin verux_apb` passed. The probe is read-only and
does not start the runner or send Arduino movement. A live Arduino movement
check has not been performed. The built app opened to its license login screen,
so its Start flow could not be exercised without the user's license session.

## Live user observation requiring investigation

After running Memory mode, the user reported that aiming repeatedly favored one
fixed world location, including when circling it, and did not track other visible
players. The user suspects it may have been a mailbox, but the object identity is
unverified. Verux APB also crashed after the user moved far from that location.
Treat both as open defects: verify the actor-class and owner relationships against
live players, reject static world objects, and obtain the crash log or dump before
claiming the source is reliable. Do not assume that one projected point matching a
player in a screenshot proves actor classification across the district.

## UE3 ownership lead and 2026-09-29 failure evidence

The published UE3 `Engine` UnrealScript declarations provide a useful ownership
model, but no APB.exe byte offsets. `Player.Actor` is a `PlayerController`;
`PlayerController.Player` points back to its `Player`; `Controller.Pawn` is the
currently possessed pawn; and `Pawn.Controller` is its reciprocal controller.
`PlayerController.PlayerCamera` is its camera reference. `WorldInfo.PawnList`
and `Pawn.NextPawn` form a pawn list; `WorldInfo.ControllerList` and
`Controller.NextController` form a controller list. `Actor.Owner` is a separate
actor-ownership field and must not be confused with the pawn's `Controller` or
with a UObject `Outer` relationship. The UE3 source is a structural hypothesis
for APB's customized build, not proof that any current numeric class marker or
offset in this repository names one of those fields.

Sources (published copies of Epic's UE3 Engine class declarations):

- [Player.uc](https://github.com/bakerm6/Src/blob/master/Engine/Classes/Player.uc)
- [PlayerController.uc](https://github.com/bakerm6/Src/blob/master/Engine/Classes/PlayerController.uc)
- [Controller.uc](https://github.com/bakerm6/Src/blob/master/Engine/Classes/Controller.uc)
- [Pawn.uc](https://github.com/bakerm6/Src/blob/master/Engine/Classes/Pawn.uc)
- [WorldInfo.uc](https://github.com/bakerm6/Src/blob/master/Engine/Classes/WorldInfo.uc)
- [Actor.uc](https://github.com/bakerm6/Src/blob/master/Engine/Classes/Actor.uc)

The local Verux log records a Memory session on PID 8244 with one discovered
nonlocal entity and two successful 15-second refreshes, followed by a refresh
failure: no class-6683 local-controller match. A later PID 21624 session began
with zero nonlocal entities, then failed the same local-controller check at its
first refresh. Repeated starts against that PID also failed to identify the
controller. These are logged source stops. No matching Windows Application
Error or Windows Error Reporting event was found in the last three days, so the
reported application crash still lacks a verified fault record.

Next read-only validation should start from the jump-confirmed local position
address in one live session. Scan bounded, readable object regions for exact
references to that address; classify each referring object's actual UE3 type
and field, then test reciprocal `Controller.Pawn` / `Pawn.Controller` links.
Independently identify the local `Player` / `PlayerController` chain and compare
its pawn pointer to the jump-confirmed address. Repeat after death/respawn,
district travel, and a full game restart. For other players, validate pawn type,
client-visible player identity, movement over time, and screen correspondence before
including their positions in targeting. Only then replace the current
class-marker/table heuristics. Keep the existing visible failure path if any
part of the validated chain disappears; do not substitute the camera-nearest
object or another guessed pointer.

UE3 source distinguishes client player identity from server controller enumeration:
[`GameReplicationInfo.PRIArray`](https://raw.githubusercontent.com/bakerm6/Src/master/Engine/Classes/GameReplicationInfo.uc)
is maintained on server and clients, whereas
[`WorldInfo.AllControllers`](https://raw.githubusercontent.com/bakerm6/Src/master/Engine/Classes/WorldInfo.uc)
documents server-only usefulness. Investigate replicated identity and current pawn
ownership for remote players; do not require a remote PlayerController to exist on
this client. These engine relationships supply hypotheses, not verified APB offsets.

## User-supplied 2025 and 2024 APB dump snippets

### Supplied globals rechecked on 2026-09-30

`supplied-globals-20260930-1.json` records a fresh EchoDriver read against
PID 19764 and the same verified executable hash. RVA `0x3D7AE30` contains
`0x138295EC36396037`; RVA `0x3DCA210` contains `0x271A9E096E26E32E`;
RVA `0x3DCA228` contains `0x5232EB441B0E9663`. These are the same nonpointer
first-qword values recorded on September 28. Their proposed GObjects, GEngine
and UWorld roles do not supply usable plain pointer walks in this build. This
does not establish encoding or justify guessing a decoding scheme.

The proposed GNames RVA `0x3CFAC90` contains `0x50D6A00`; its pointee begins
with module vtable `0x1431E1118`, consistent with the earlier observed
object-like pointer run. GNames semantics remain unverified. The latest labeled
respawn survey found no markers 1015, 6714 or 5776 in the explicit diagnostic
window. Marker 16845 did occur at two addresses, both with vtable RVA
`0x2B87490`; this is a HUD-type hypothesis, not confirmed HUD ownership.

The supplied +0x3C location aligns with the marker field used by the diagnostic,
while marker identities differ for the verified camera (16920). +0x88 aligns
with controlled position evidence for type RVA `0x3142FB0`, marker 4741.
+0x680 aligns with the verified view/projection block, but that block was
validated in the camera object, not a supplied HUD. Current camera position
is +0x6C0, not verified at the supplied +0x480. PlayerController.Pawn +0x790,
GameInstance +0x3FC, and the zero-valued health/mission/faction/death offsets
remain unverified. Zero placeholders are not accepted field definitions.

The 2025 C++ snippet uses configurable `Offset_GNames` and `Offset_GObjects`
values but does not supply their numbers. The 2024 C# snippet supplies absolute
addresses `GNames = 0x14390DFE0` and `GObjects = 0x14398E150`. Relative to the
`0x140000000` image base observed in the 2026 sessions, these would be RVAs
`0x390DFE0` and `0x398E150` respectively. Neither snippet gives an executable
hash or evidence that those addresses still describe the current build.

Both snippets interpret each global as an array header: a pointer to entries
at `+0x0`, an `int32` count at `+0x8`, and an `int32` capacity at `+0xC`;
entries are eight-byte object or name pointers. They read an object's class
pointer at `+0x18`, name index at `+0x24`, and `Outer` pointer at `+0x2C`.
For names they inspect a flag at the name entry and select an inline string at
`+0x18` or an indirect string pointer at `+0x30`. These are layout claims to
test by checking bounds, pointer readability, decoded class/name consistency,
and multiple known objects in the verified 2026 process. An earlier native Memory
source scanned 32,768 pointers directly from `base + 0x3CFAC90`;
that was a different interpretation and location. The current runtime now uses
the rooted traversal described in the native integration section above.

`UObject.Outer` in these snippets supplies package/name containment. It does
not identify the actor that owns or possesses a pawn. Even if this older
GObjects/GNames scheme is validated, it only improves type identification;
the local `Player` -> `PlayerController` -> `Pawn` relationship and the world
pawn list still require separate APB-specific validation. The C# helper also
returns zero on failed reads, which would hide an invalid pointer, so its error
handling must not be copied into the read-only diagnostic or product source.

## 2026-09-30 offline rooted-list evidence

The saved identity captures now support a concrete candidate roster route:
Camera +0x2E0 -> World, World +0x48C -> first pawn, and pawn +0x350 -> next
pawn. Six captures in PID 19764 produce NULL-terminated chains covering all
captured character pawns, with counts 6, 6, 7, 8, 7, and 2. Each reached pawn
points back to the same World. This is agreement with the saved survey subset,
not proof of a complete district or visible-player roster.

The earlier identity diagnostic scanned pointer fields at eight-byte steps and
therefore skipped the four-byte-aligned +0x48C field. Target pointer alignment
must be distinguished from pointer field alignment. Other saved annotations
also show vehicles at the candidate list head and character +0x350 links to
vehicles, so traversal must handle mixed pawn types before selecting players.

See [APB_RELATIONSHIP_REVIEW.md](APB_RELATIONSHIP_REVIEW.md) for raw chain evidence,
address-versus-position comparisons, possession transitions, published source
correlations, pseudo operations, and a reproducible offline command. Numeric
offsets remain tied to executable hash
`3BECA02821F50A7B8FEF3E16923FA3FCA0D0B0DD49CC9622520EA500001E1640`.
Rooted mixed-chain coverage across launches, districts and respawn remains
unverified. The subsequent runtime integration uses these fields with strict
node/type/World/fingerprint checks; it does not claim live completeness.

### Native saved-capture replay

The existing read-only probe now has an explicit offline mode that uses the same
Rust roster reader as the product, rather than duplicating its traversal:

```powershell
cargo run --offline --example apb_memory_probe -- --capture-root "C:\Users\Admin\Documents\source_repos\drayebr\build-rpm\snapshots"
```

Replay reached 6, 6, 7, 8, 7, and 2 characters and matched all captured character
fingerprints. The list traversal required 14, 14, 16, 18, 16, and 6 reads,
respectively, totaling 12,616, 12,616, 14,328, 16,040, 14,328, and 5,768 bytes.
These counts exclude camera lookup, per-frame sampling, and actual driver
latency. The captures contain no complete mixed rooted chain to replay.

The identity inspector also now checks pointer fields at four-byte steps, with
an eight-byte read bounded by the saved object prefix. Its pointer-target
alignment check is unchanged. No new automated tests or fixtures were added.
