# Memory coordinate source status

The EchoDriver repository (`..\drayebr`) now has a read-only process-memory
IOCTL and an administrator-only diagnostic client. `src/memory_reader.rs` is the
matching Verux transport. Neither component writes to APB; Verux mouse movement
remains on the existing Arduino serial path.

The Misc tab has a **Refresh APB PID** button next to the COM-port refresh. It
enumerates processes named exactly `APB.exe`, opens a query handle to verify the
candidate, and displays the PID from that refresh. It excludes similarly named
processes such as `APB_Catcher.exe`. The PID is transient and must be refreshed
after the game restarts; discovery alone does not validate game offsets.

Memory mode now starts for the exact APB executable hash recorded below when
EchoDriver is available to an administrator process. It scans for a validated
camera and local controller, projects other player-linked positions, and
provides a centered point to the runner's existing Arduino movement path.
The current selection includes neutral players and has no opponent, occlusion,
or dead-player filter. The user explicitly deferred opponent filtering until
a combat-district capture is available. Color detection remains a separate
explicit source and is never used automatically if Memory fails.

## Verification checklist and current limits

1. Record the APB executable and module name, build identifier, and runtime
   module base. Treat supplied absolute addresses as observations from one run;
   resolve the supplied relative offsets against the current module base.
2. Verify the `GObjects` layout and a repeatable pointer walk to the local
   controller, local pawn, another player pawn, camera, and HUD. Record each
   field's type and whether it is an inline value or pointer.
3. Verify the HUD view matrix layout, multiplication convention, and viewport.
   Compare at least one known world position with its observed screen position.
4. The local controller is excluded by its validated class-6683 link. Other
   controller-linked and owner-linked class-4741 positions are candidates.
   Teammate, opponent, dead-player, and occlusion filters remain future work.
   The supplied zero offsets for health and faction are unknown, not field
   definitions.
5. The native source samples positions synchronously each runner frame and
   returns a centered one-pixel target point. It rejects invalid camera/local
   reads with a visible logged error; actors that leave are logged once and
   removed by the periodic discovery refresh.
6. Memory points use the runner's existing activation, smoothing, braking, and
   Arduino path. Memory errors stop the source; no image-detection switch
   occurs. Triggerbot remains visual red detection, as shown in the UI.

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

`src/memory_projection.rs` contains the checked row-vector projection for
these camera samples. It returns APB client-relative pixels, rejects invalid
or nonfinite data, and reports points behind or outside the view separately.
`src/apb_memory.rs` uses it to produce the runner's centered player point.

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
