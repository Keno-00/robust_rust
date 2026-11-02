# Live validation overlay

This is an overlay-only diagnostic for the captured 2026-10-18 executable
`35AEF01734AAB20B4B0C622ADDF5706F9E75CAAA98D5B84A0EA14D9DF72EE453`.
It uses EchoDriver and an explicit profile from the same APB process launch.
It does not update production offsets or approve the build hash.

Launch from elevated PowerShell with a new output directory:

```powershell
.\scripts\run_apb_validation_overlay.ps1 -Profile captures/apb-level-20261018-streamed-e9c009d1/profile.json -OutputDirectory captures/new-live-validation
```

Focus APB. F8 starts/stops. F4 cycles Players, Objects and Cursor. F11 exits.
Only one overlay should run so those hotkeys do not trigger another diagnostic.

| Mode | Controls | Display/evidence |
| --- | --- | --- |
| Players | Aim near a marker; F5 cycles nearby pawns; F6 locks the target. F1 labels exposed, F2 blocked, F3 unknown. | Green RECENT or yellow STALE with timestamp age. Locked targets can be labelled while out of view. The candidate is render recency, not geometry line of sight or permission to shoot. |
| Objects | F3 switches pumpkin/grenade tags; F5 cycles nearby markers; F6 annotates; F7 undoes. | Object name/address, operator tag and first-seen age for live grenade/projectile name candidates. Names such as a held grenade weapon remain distinct from confirmed thrown grenades. |
| Cursor | F1 records white; F2 records green over the user's car; F3 records red only when available. | State-correlated pointer-path candidates and count, with exact labels and raw values saved. Repeat white/green reversals; uncaptured red behavior remains unverified. |

Local state, capsule half-height and radius are shown in every mode. Turning,
strafing, standing/crouching and entering/exiting a vehicle produce live samples
and ownership-binding events. No posture name is inferred solely from a height.
Changing root/slot/fingerprint snapshots are withheld, logged and sampled again
through the same rooted reader. An unsupported layout, invalid geometry or
driver error stops with an actionable message; no alternate source is chosen.

## Object lifetimes and timestamps

The current persistent-level actor array is surveyed about once per second in
Objects mode. Object/class name matches for `grenade` or `projectile` expose
new instances without a full module/heap rescan. Each survey guards array
membership and fingerprints before/after reads. A changing membership rejects
that survey, clears its displayed transient candidates and requests a fresh
snapshot. Existing profile props are reread and expired instances are removed
explicitly; their tags do not transfer to replacement objects.

`first_seen`, `last_seen`, annotation UTC, host monotonic time and available
World-clock values are recorded separately. Survey prefixes up to `0x1000`
bytes are bounded probes, not claims about allocation size. These timestamps
do **not** establish exact spawn, fuse or detonation time. A missing instance
records loss from the surveyed actor list, not proof that it exploded.

For pumpkins, tag the same location before collection, after collection and
after respawn; state which condition each capture represents. A persistent
audio/prefab location is not automatically proof that the pickup is available.

## Scope of confirmation

The user confirmed the outside-mission case with token `0xFFFFFFFF`. They cannot
join a mission for this run; active teammate/opponent behavior stays pending.
The current build's clock/timestamp structure was checked separately in the
runtime audit. User exposed/blocked labels in this overlay remain independent
of the candidate and permit disagreement to be reviewed rather than hidden.

Cursor probing is explicitly bounded to module RVAs `0x3BB3360..0x3BF3360`,
following four-byte fields at pointer `+0x134`. This region is centered around
the old cursor root shifted by the observed HUD-root delta; that is a search
hypothesis, not a rule that all offsets share one delta. Read failures and
empty results are retained. The probe never silently switches to a heap scan.

The append-only `events.jsonl` records samples, operator observations, tags,
undo, cursor captures, first/last observations, rejected snapshots and errors.
F8 retries within the existing process. After a full APB restart, close this
overlay and create a fresh process-identity profile before relaunching.

No new automated tests or fixtures were added. Live startup and the user's
subsequent observations are the validation evidence; structural plausibility
is not reported as behavioral confirmation.
