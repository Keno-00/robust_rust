# APB build candidates — 2026-10-18

Discovery executable SHA-256:
`35AEF01734AAB20B4B0C622ADDF5706F9E75CAAA98D5B84A0EA14D9DF72EE453`.
Process 6376, module base `0x140000000`, image size `0x537A000`.
All process-memory bytes were read through EchoDriver v3 in elevated helpers.
The scanner-only `APB_SCAN_ALLOW_UNVERIFIED_BUILD=1` override was logged.
At discovery time no production constants were changed. The subsequent
user-requested [core update](APB_CORE_UPDATE_20261018.md) applies the captured
core roots/types and executable hash, with the remaining limitations recorded.

## Candidate offsets

| Item | Prior build | New candidate | Evidence and limits |
| --- | --- | --- | --- |
| HUD/projection table root RVA | `0x3DBF110` | `0x3DCE240` | Root contained three entries; entry 1 reached the unique matrix-shape match in this capture. Restart/district checks pending. |
| HUD vtable RVA / name index | `0x2B5EEA8` / 16920 | `0x2B67088` / 16929 | Name-entry probe returned `cHUDBase`. Instance `0x9C42EB00`. |
| HUD matrix / camera XYZ | `+0x680` / `+0x6C0` | `+0x680` / `+0x6C0` | Finite matrix, element 14 near -9.99, element 15 near zero; live values reread. Visual correspondence remains provisional. |
| WorldInfo vtable RVA / name index | `0x3161E78` / 430 | `0x316FF28` / 430 | Followed HUD `+0x2E0` into a typed object; bounded raw body retained. |
| Controller vtable RVA / name index | `0x2A7F158` / 998 | `0x2A87258` / 998 | Followed WorldInfo `+0x358`; possession field `+0x790` points to the observed local pawn. |
| Pawn vtable RVA / name index | `0x2AAEFA0` / 5778 | `0x2AB7090` / 5778 | Name-entry probe returned `cAPBPawn`; XYZ candidate remains `+0x88`. Ownership/lifecycle scenarios pending. |
| Prefab instance vtable RVA / name index | `0x3142FB0` / 4741 | `0x3151050` / 4741 | Name-entry probe returned `PrefabInstance`; XYZ candidate `+0x88`. This name does not establish pumpkin identity. |
| Name-array root RVA | Not established here | `0x3CD1F00` | Pointer/count/capacity shape; sampled entries have index flags at `+0x8`, matching `flags >> 1`, and name text at `+0x18`. |
| Level vtable RVA / actor-array offset | Not established here | `0x3208BF8` / `+0xB4` | WorldInfo outer pointer `+0x44` reached this type. One array contained WorldInfo; 13 captured levels supplied actor slots. |
| Collision component vtable RVA / name index | `0x3197448` / 3269 | `0x31A54F8` / 3269 | Local pawn `+0x248` reached this type. Reciprocal owner links and product geometry checks passed in the runtime audit below. |
| Cursor pointer root / field | `0x3BC4230` / `+0x134` | `0x3BD3340` / `+0x134` | Eight operator-labelled green/white captures narrowed 1,190 initial leads to one pointer path; green 48 and white 130 repeated. Red and restart behavior pending. |

## Captures

- Broad capture: `captures/apb-build-20261018-3408f06c/candidates.json`.
  Captured writable module sections and 2,004 array-root hypotheses, inspected
  178,697 distinct referenced addresses, retained 154,943 module-vtable
  prefixes and 5,824 provisional XYZ candidates. This was bounded: arrays
  above 131,072 entries were truncated and 2,018 object reads failed. Exact
  per-root bounds and failures are retained in the report.
- Local link recheck: `captures/apb-build-20261018-3408f06c-local/recheck.json`.
  Captures the live HUD, WorldInfo/controller bodies and selected name entries.
- Current-level capture: `captures/apb-level-20261018-fc2d6926/`.
  Located the actor-array candidate through WorldInfo membership.
- Streamed-level capture: `captures/apb-level-20261018-streamed-e9c009d1/`.
  Inspected 7,414 actor slots across 13 level objects, retained 7,401 unique
  bounded object prefixes and 140 XYZ candidates within 5,000 world units of
  the captured camera. One level with no actor slots was explicitly recorded.
  The profile includes generic `prop_...` names and prefab instances, without
  asserting that these objects are pumpkins.
- Overlay profile: `captures/apb-level-20261018-streamed-e9c009d1/profile.json`.
- Current annotation session: `captures/pumpkin-annotations-20261018-0444e786.jsonl`.
  The first session (`pumpkin-annotations-20261018-c548ac3b.jsonl`) stopped
  when an old weapon instance expired. The corrected overlay removes replaced
  candidates, invalidates their tags and reports their exact keys in the log
  and a visible counter. The current session reached live operation and logged
  those removals; camera/read failures still stop projection.

The first saved F6 tag is `pumpkin` on `0x18BFFF580`, name index 16985,
`cAudioEmitterPositioningActor`, vtable RVA `0x2AAD618`, XYZ at `+0x88`.
Its recorded position is `(99169.8671875, 148593.640625, 516.10009765625)`.
This is an operator association, not proof that the actor is the pumpkin mesh.
The captured prefab instance `0x185101400` is 38.11 world units away at
`(99169, 148594, 478)`, vtable `0x3151050`, name index 4741, position `+0x88`.
This nearby prefab is a separate candidate for visual confirmation.

## Pumpkin annotation follow-up

The later annotations identify two spatially matching groups:

| Group | Audio emitter | Prefab instance | Extra actor, name index 1650 |
| --- | --- | --- | --- |
| A | `0x18BFFF580` | `0x185101400` | `0x24B80B580` |
| B | `0x18D88A080` | `0x185100C00` | `0x2416EE080` |

In the streamed-level raw bodies, both emitters have a pointer at **`+0x274`**
to the corresponding prefab instance. This is a direct recorded relationship,
stronger than proximity alone. It has not yet been rechecked after collection
or respawn. The emitter and prefab share a captured level outer pointer;
the extra actors belong to the persistent level instead.

For each group, the extra actor's X/Y match its prefab exactly and its Z is
one world unit lower. Both extra actors share vtable RVA `0x2B3A290`, name
index 1650 and a shared pointer at `+0x30` to `0x1825B0B80`. Their text name read as `'1f.`;
no semantic class name or activation meaning has been established.

The repeated saved annotations show:

- Prefab `0x185101400`: no differences within the captured first `0x100`
  bytes between 04:13:23 and 04:13:41 UTC.
- Extra actor `0x24B80B580`: its four-byte field at `+0x80` changed from
  `0x10050384` to `0x100D0384` between 04:13:21 and 04:15:09 UTC, a change
  of bit `0x00080000`. Pointer-like fields and fields at `+0xB4`/`+0xB8`
  changed as well. These differences are candidates only: the annotations
  do not label collected/available states, so none establishes activation.

Distinguishing an active pumpkin from a persistent location requires matched
before/after captures at the same spot, ideally repeated through a respawn.
No activation predicate or automatic pumpkin classifier has been added.

These captures establish candidates for this launch. Cursor colors, visibility,
health, mission fields, vehicle/passenger ownership and restart stability have
not been validated for the new executable. A matrix-shaped HUD alone does not
establish correct screen projection; use the overlay to confirm correspondence.

## Runtime audit and confirmation

`captures/apb-runtime-audit-20261018-0495dca7/audit.json` records six passing
structural checks for the same process/build. This supersedes the earlier
unchecked status for these particular snapshot checks; it does not establish
all behavioral or lifecycle cases.

| Check | Recorded result | Remaining confirmation |
| --- | --- | --- |
| HUD/projection binding | Exactly one expected HUD type in 128 table slots; selected slot, root and fingerprint rechecked; matrix shape passed. | Look up/down, turn, strafe and compare markers to visible objects; district/restart reacquisition. |
| Local ownership | Current on-foot controller/pawn backlinks, World link and XYZ passed. | Vehicle entry/exit, passengers, death/respawn. |
| Rooted roster | World `+0x48C`, pawn/vehicle next `+0x350`, and World backlink `+0x2E0` passed; six pawn nodes and 21 vehicle nodes; current local pawn present; every edge/fingerprint reread. | Active/dead/streamed lifecycle membership and reacquisition. Counts are nodes, not a claim of six active players. |
| Collision geometry | Pawn `+0x248` reached `0x31A54F8` / 3269. Owner backlinks `+0x44` and `+0x7C` passed. Half-height 91, radius 34, extents `(34,34,91)`; sphere matched the extent length; stance fields reread. | Standing/crouched visual correspondence and remote pawn checks. |
| Timestamp consistency | World double `+0x6A0` advanced from 2778.9978723 to 2779.5272696 seconds. Pawn doubles `+0xB4` were finite, nonnegative and no later than the clock tolerance in three observations. | Exposed/hidden/looking-away controls. Render recency does not itself prove an unobstructed shot. |
| Mission block | Six pawn blocks at `+0x990..+0x99F` were stable across double reads and identity checks. Local token was `0xFFFFFFFF`. | User confirmed being **outside a mission** during this audit, supporting that one case. Active same-side/opposite-side and mission transitions remain unverified. |

The outside-mission label came directly from the user after the audit. No
in-mission state, cursor color, health value, occlusion state or collection
state was inferred from names alone. No production constants were changed.

For the next confirmation run, use explicit settled states and keep their
labels with the captures:

1. Projection and pose: turn/look up/down, walk/strafe, stand/crouch.
2. Ownership and lifecycle: enter/exit a vehicle, passenger case, death/respawn,
   district travel, then full APB restart.
3. Mission and cursor: outside/inside a mission with known teammate/opponent;
   white/red/green cursor states.
4. Visibility: the same stationary remote pawn exposed, behind opaque cover,
   and outside the view, with repeated reversals.
5. Pumpkin availability: the same location before collection, after collection
   and after respawn; compare emitter/prefab links and the extra actor's fields.

Reproduce the structural audit from an elevated shell, using a new directory:

```powershell
$env:APB_SCAN_ALLOW_UNVERIFIED_BUILD = '1'
$gameProcessId = (Get-Process APB).Id
python -u -B scripts/apb_runtime_audit.py $gameProcessId captures/apb-level-20261018-streamed-e9c009d1/profile.json captures/new-runtime-audit
```

The audit requires a profile from the exact process launch and the captured
executable hash. A restart requires fresh identity-bound discovery evidence.
Each feature check reports its own failure; a failed rooted roster is not
replaced by level arrays or a different memory reader.

## Interactive validation follow-up

`captures/apb-validation-live-20261018-392e67cf/events.jsonl` retains the
interactive player, object and cursor observations. Eight cursor labels were
green/white repeated four times; the candidate root narrowed from 1,190 to
two, then one. The remaining root `0x3BD3340` pointed to `0x9309D780` in this
launch, with `+0x134` reading 48 for every green label and 130 for every white
label. This is state correlation for two colors, not red or restart approval.

The same player `0x161DF8100` received blocked labels at timestamp ages
6.2853904, 3.439643 and 0 seconds, and an exposed label at 0 seconds. The
zero-age blocked observation disagrees with the one-second RECENT display;
its settled-state timing is not established, and the discrepancy is retained.
No geometry-visibility predicate was accepted or retuned from these labels.

The live object survey recorded `cGrenadeWeapon` and two newly appearing
`cAPBProjectile` instances, `0x23BA9FC00` and `0x157CBA400`, with observation
times and bounded raw prefixes. These are grenade/projectile name candidates,
not proven fuse clocks or exact spawn/detonation times. Names from the related
pointer at `+0x30` were historically logged as `class_name`; the semantic role
of that pointer has not been established and those strings are supplementary
evidence only.

The first overlay rejected changing adjacent HUD counters as binding changes.
Before/after evidence showed the same table pointer with a different count.
The current guard validates counts separately, and guards the actual selected
slot, object fingerprint and unique HUD across repeated active-entry reads.
Changed or ambiguous samples remain withheld and logged through the same reader.
See [APB_VALIDATION_OVERLAY.md](APB_VALIDATION_OVERLAY.md) for controls.

## Discovery commands

From an elevated PowerShell at the repository root, with fresh output paths:

```powershell
$env:APB_SCAN_ALLOW_UNVERIFIED_BUILD = '1'
$gameProcessId = (Get-Process APB).Id
python -u -B scripts/apb_build_candidates.py $gameProcessId captures/new-build
python -u -B scripts/apb_level_candidates.py $gameProcessId captures/new-build/candidates.json captures/new-levels --radius 5000 --name-array-root 0x3CD1F00
.\scripts\run_apb_object_annotation_overlay.ps1 -Profile captures/new-levels/profile.json -Output captures/new-pumpkin-tags.jsonl -Tag pumpkin
```

Use the explicit name-root candidate only for this executable. The level scan
identifies the persistent actor-array candidate, then checks the same offset
on captured objects with the exact level vtable and fingerprint. Its component
array and XYZ offsets remain recorded hypotheses. Changed array headers reject
the capture; no alternate memory reader or guessed replacement root is used.

Overlay controls and evidence format: [APB_OBJECT_ANNOTATIONS.md](APB_OBJECT_ANNOTATIONS.md).
No automated tests were added or run. The discovery scripts and overlay startup
were exercised against the live process, and the running overlay saved an F6
annotation. Visual correspondence, pumpkin mesh identity and restart stability
remain unverified.
