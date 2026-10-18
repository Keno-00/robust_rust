# Green pumpkin state and marker continuity — October 18, 2026

The user clarified that green pumpkins use possible locations with a daily selection; red locations are fixed. Asset identity therefore identifies a possible placement, not whether a green pumpkin currently exists there.

## Labelled controls

`captures/pumpkin-visible12-empty29-before-r2.json` records current PID 22492:

| Operator label | Prefab | Persistent prop | Camera distance |
|---|---|---|---:|
| Visible, intact green | `0x1A655E000` | `0x1705C0100` | 12.26 m |
| Empty green location today | `0x1A655DC00` | `0x1757BB580` | 29.74 m |

Both locations have the same pumpkin asset identity and a persistent prop, type `(0x2B3A290, 1650)`, positioned one world unit below the prefab. Thus neither prefab nor prop existence alone establishes availability.

## Controlled destruction

The user shot the visible green pumpkin during `captures/pumpkin-visible12-empty29-shot.jsonl` and confirmed it broke. The recorder saved 240 samples with bounded prop/component bodies. Derived counts and transitions are retained in `captures/pumpkin-visible12-empty29-shot-summary.json`.

| Prop field | Intact target | Target after shot | Empty control |
|---|---|---|---|
| `+0x7C & 0x4000` | Clear, 123 samples | Set, 117 samples | Set, all 240 |
| `+0x84 & 0x80` | Clear, 123 samples | Set, 117 samples | Set, all 240 |

Both flags changed together at 17:40:41 Manila time, World time 4662.3760672. Both objects retained their exact fingerprints throughout; the two bits never disagreed in either subject. These are observed hidden-state masks, not claimed reflected property names. The `+0x80 & 0x10000000` bit fluctuated after the shot and is deliberately excluded from availability decisions.

This confirms one intact-to-destroyed transition against an unchanged empty control. Daily reselection, reactivation and red destruction have not been observed. Raw component capture sizes are bounded probes, not established allocation sizes.

## Native green eligibility

The classifier retains identity checks through exact prefab type, the current `+0x308` asset reference and dynamically resolved `PumpkinGreen_prop` name. A fresh, unique prop association must also be present in the current persistent-level actor array:

- Exact prop type and fingerprint.
- Matching World backlink.
- Position within three world units of prefab XYZ plus `(0,0,-1)`.
- Both observed hidden masks clear.

Unknown, absent, ambiguous or hidden green props do not produce a marker. Each rendered green marker rereads this prop and repeats the checks after prefab/asset validation, so destruction can remove it without waiting for another placement discovery sweep. Inactive prefab identities remain cached, allowing a future state change to be evaluated without requiring a new actor-array insertion. Red locations retain the user's requested fixed, unconfirmed-location behavior.

## Marker flicker correction

The previous survey discarded an entire level's cached prefabs whenever any actor changed, then waited for that level's turn in an eight-level rotating discovery budget. This could hide valid pumpkins until rediscovery.

The corrected budget applies to finding new objects. Existing cached pumpkin identities are intersected with fresh membership and individually reread. Unrelated actor changes do not discard valid survivors. If membership changes during a pass, surviving instances are checked again against the later snapshot; incoherent snapshots still withhold affected markers. World changes, unloading, changed identities, failed guards and foreground loss continue to withhold output. No stale-position or alternate-reader fallback is introduced.

The earlier native log also recorded `Pumpkin asset name binding changed during lookup` while loading another area. The old guard compared the entire GNames header, rejecting unrelated count/capacity growth. Its replacement validates the latest table's bounds and confirms that this exact index still points to the same verified name entry. Entry flags, exact asset text, asset fingerprint and current slot identity remain checked; unrelated name-table growth no longer rejects a valid selected entry.

No automated tests were added or run. The controlled shot is live operator-labelled diagnostic evidence; compilation and native visual acceptance of the new green filter are reported separately.

## Replacement build

`cargo build --locked --release --bin verux_apb --target-dir target/v3-release` completed successfully in 7m 28s. The executable was rebuilt at approximately 17:51 Manila time and launched through the approved administrator path as PID 23284. Artifact size: 14,341,120 bytes; SHA-256 `D4A0289DB93EBA7EE8B8AD813E649139BE857FBBB25D2EF12937BC017B8B5462`.

This replaces the prior pumpkin-location-only client. Visual confirmation of the empty, destroyed and remaining active markers is still pending in this rebuilt client; the controlled memory transition itself was captured in the previous run.

The final name-table growth guard correction was then compiled successfully in 7m 16s at approximately 18:00 Manila time. Current executable SHA-256: `0EE8D3AA3C6ECE1395C424CFC7B5E032AF5E3EF0C13E4CC89CA3B3A8D50EF942` (14,341,120 bytes); approved administrator launch PID 18116. This is the current replacement artifact, superseding the intermediate 17:51 build.

Native acquisition restarted at approximately 18:01 Manila time. Both workers published their first verified batches. Later logs showed individual revalidation during level membership changes, conservative withholding of one location without a unique prop association, and removal of an ineligible green marker. No object-acquisition fatal error appeared in the inspected current-run interval. These logs establish that the new paths execute; they do not replace visual confirmation of all markers or a daily-reselection test.
## User confirmation and radar follow-up

The operator confirmed that green markers appeared and that killing green pumpkins removed their markers. Daily reselection and red visual identity remain unconfirmed.

The same run stopped at log time 10:02:42 UTC with `Object acquisition stopped: Invalid memory read: nonzero PID, user address, and 1-4096 bytes required.` No matching Application Error/WER record was returned by the inspected two-hour event query. The old message did not identify the address or call site, so the exact trigger cannot be proven from it. Inspection found an unguarded prefab asset reread in the availability pass: a detached asset could reach the reader as a null address. This path now validates the pointer and explicitly logs/withholds detached markers. Reader rejection diagnostics now include PID, address and length; genuine failures remain visible.

Added saved `comet_jade` setting, exposed as Pumpkins under Radar with a distinct ImGui ID. It enables the existing pumpkin worker independently of world-space Pumpkin markers. Radar consumes verified world positions, including objects behind the camera, within its existing 100 m horizontal range. Acquisition radius includes the camera/local-origin separation; world markers retain their 80 m camera range. Green availability checks and red candidate coloring are shared. Existing encrypted u133 text is reused; no server catalog change is required.

Release compilation and renewed native acceptance are pending for this follow-up. No automated tests were added or run.

Follow-up release build succeeded in 7m 54s. Executable SHA-256: CC936DDE3ABB10B288D36A8C45434F79CDFE842AAF1F106F05167351F9C27C7B. Administrator launch was requested through Start-Process -Verb RunAs. Live radar behavior and the acquisition-stop correction still require operator confirmation.

## Red availability correction

Operator labels: nearer red pumpkin about 17 m intact; farther red pumpkin about 27 m already smashed. `captures/pumpkin-red-intact-smashed.json` captured both with stable persistent actor membership and zero read errors. Camera distances at capture were 15.93 m and 24.37 m (camera differs from local-pawn distance).

Intact prefab 0x1A156E800 has unique cProp 0x167DA7580, +7C=0x85848452 and +84=0x4008; hidden mask 0x4000 and disabled mask 0x80 are both clear. Broken prefab 0x1A156A400 has unique cProp 0x1955C9600, +7C=0x8584C452 and +84=0x4088; both masks are set. Both use the same verified cProp type and match the existing position association tolerance.

Removed the red fixed-location availability bypass from survey publication and per-frame eligibility. All pumpkin colors now require their unique associated prop with both live-state masks clear, including the second per-frame check. This affects world markers and radar through their shared feed. Missing/ambiguous props remain withheld visibly in the log. This labelled intact/broken comparison supports red availability; a controlled red before/after kill and daily reset are not yet captured. No automated tests were added or run. Replacement build pending.

Red-filter release build succeeded in 7m 06s. Executable SHA-256 F48865A567018F64B5A4697334FE67B580F3C9788A41F851496E1A8709D8D2C8; administrator launch PID 21828. Native visual confirmation of the new red filter remains pending.

## Encrypted reader session exhaustion

The red-filter client stopped at 10:55:33 UTC (18:55:33 Manila) with `Object acquisition stopped: EchoDriver session limit reached; reconnect`. No matching Application Error/WER record was returned by the inspected one-hour query. This is a fail-visible acquisition stop after reaching the v3 transport read count, not evidence that pumpkin destruction flags caused a native exception.

Inspection of the authoritative adjacent driver source (`../drayebr/driver/Driver.c`) confirms a per-file session: IOCTL_SESSION only establishes a random identity/key for an unestablished file context; querying an established handle does not reset its replay counter. Authenticated read sequences are restricted to 1..0xFFFFFF. The client previously stopped at that bound and had no scheduled renewal.

`AtlasReader` now owns its device handle inside the mutex-protected Session. Before reaching the driver limit (1024 requests of headroom), it opens a fresh handle through the existing v3 handshake, requires initial driver sequence zero and an identity different from the previous session, derives the fresh key, logs renewal, then replaces/drops the old session and closes its handle. All requests remain serialized through the same mutex. No failed request is retried, no nonce counter is reset under an old key, and no alternate reader or relaxed authentication is introduced. Failure to renew is an explicit processing error. The driver's limit and response validation remain unchanged. This shared transport covers the object, core and cursor readers.

No automated tests were added or run. Release compilation and workload-driven rollover confirmation are pending.

Session-renewal release compilation succeeded in 6m 24s. Executable SHA-256 7C1B4F0F1B9C60F3409561D4C3394F12F1D6692BE9D5A884271F477AAC7AD779. Administrator launch requested; natural-workload rollover still needs live confirmation.
