# APB core update — November 24, 2026

APB patched on November 24 (`APB.exe` written 21:59 Manila time). The user requested updating Verux.APB and Verux.SM for the new build. Normal builds now require executable SHA-256 `A4EA425A995BBD97A8789BF98A5E7E63E5D945FC9E193462B60375F070223742`. The October 18 executable (`35AEF017…E453`) is no longer accepted by the normal hash gate. `apb-hash-test` is unchanged and not used for this release build.

## Applied constants

Each `ObjectType` is (vtable RVA, name index at `+0x3C`).

| Definition | Previous | New | Evidence |
|---|---|---|---|
| Executable SHA-256 | `35AEF017…E453` | `A4EA425A…3742` | Installed `APB.exe`, running PID 18384 |
| Camera table RVA | `0x3DCE240` | `0x3FD64E0` | Global whose array holds the live HUD at slot 1 (same slot) |
| HUD (`cHUDBase`) | `0x2B67088` / 16929 | `0x2D053F8` / 17072 | Live instance; matrix `+0x680` element 14 = -9.99, element 15 = 0 |
| WorldInfo | `0x316FF28` / 430 | `0x332ACF8` / 430 | HUD `+0x2E0` |
| Controller | `0x2A87258` / 998 | `0x2C13D28` / 1015 | WorldInfo `+0x358` |
| Pawn (`cAPBPawn`) | `0x2AB7090` / 5778 | `0x2C494A0` / 5780 | Controller `+0x790` possession; also in roster |
| Vehicle (`cAPBVehicle`) | `0x2B320A8` / 6064 | `0x2CC4C88` / 6080 | Roster nodes (WorldInfo `+0x48C`, next `+0x350`) |
| Collision (`CylinderComponent`) | `0x31A54F8` / 3269 | `0x33602A8` / 3271 | Pawn `+0x248` |
| Cursor pointer RVA / field | `0x3BD3340` / `+0x134` | `0x3DDB5A0` / `+0x134` | Operator aimed at own car and away: 48/130 alternated 60+ times in 60 s |
| Name root RVA | `0x3CD1F00` | `0x3EDA180` | Entry identity `flags >> 1`, entry 0 `None`, 64/64 samples |
| Level | `0x3208BF8` / 1160 | `0x33C3BB8` / 526 | WorldInfo `+0x44`; 257 live levels share this pair |
| Level owner (`World`) | `0x3164988` / 1151 | `0x331F518` / 1032 | Persistent level `+0x44` |
| Prefab instance | `0x3151050` / 4741 | `0x330BBE0` / 4743 | Name 4743 reads `PrefabInstance`; 18,000+ live instances |
| Prefab asset vtable | `0x31517A8` | `0x330C338` | Prefab `+0x308`, class `Prefab`; same `+0x758` gap from the instance vtable as before |
| Prop (`cProp`) | `0x2B3A290` / 1650 | `0x2CCCE60` / 592 | See below |
| Projectile (`cAPBProjectile`) | `0x2B68D90` / 6770 | `0x2D070F8` / 6782 | `Default__cAPBProjectile` vtable; name 6782 reads `cAPBProjectile` |

Unchanged and rechecked: matrix `+0x680`, camera `+0x6C0`, pawn position `+0x88`, HUD `+0x2E0`, WorldInfo `+0x358`/`+0x44`/`+0x48C`, controller `+0x790`, pawn `+0x248`, roster next `+0x350`, cursor `+0x134`.

## How the values were found

All memory was read through EchoDriver v3 (elevated) with Songmaker's reader, `apbsongmaker/examples/apbm_remap.rs`. That tool searches the image's writable section for the name root and object hash, walks all objects (about 420,000 in district), and groups them by class name and (vtable, marker). An instance's marker is its class object's name index. This was confirmed for HUD, pawn, vehicle, projectile, prefab and cProp, so a class with no live instance takes its `Default__` object's vtable and its class name index.

**cProp:** no pumpkin props were present (no actor at any generic prefab position with Z−1). In the November 4 capture, the old prop vtable `0x2B3A290` also hosted marker 6173, which is followed by nine `cPropComponent_*` defaults (6174–6182). In this build `Default__cProp` is 6189, followed by the same nine components (6190–6198), with vtable `0x2CCCE60` among the component vtables. Its class object's name index is 592. No live `cProp` instance was observed. The pumpkin feature's prop matching is structurally derived but not live-confirmed.

Low engine-range name entries (WorldInfo 430, controller 1015, level 526, World 1032, cProp 592) do not decode as text with the FName layout used for game names. Those types were established through structural links and class objects, not name text.

## Limits

- Red cursor 99 is retained as before, not observed on this build.
- Mission fields (`+0x990`), render timestamps, district change, restart, passengers and respawn were not exercised.
- Pumpkin props were absent; `PROP` is derived as described above.
- The native client was not launched against APB in this session.
- Diagnostic Python scripts keep their historical hash/identity convention and constants, as on October 18.

## Evidence

`docs/internal/evidence/apb-core-20261124/`: `remap-report.txt` (final full pass, PID 18384, creation FILETIME 134400024172400032, base `0x140000000`), `cursor-leads.txt` (39 scans) and `cursor-root-0x3DDB5A0.txt` (50 ms samples while the operator aimed at their car and away).

## Build

```powershell
cargo build --locked --release --bin verux_apb --target-dir target/v3-release
```
