# World object annotations

## Nearby emitter/prefab capture (2026-10-18 discovery build)

With APB foreground, run in elevated PowerShell:

```powershell
$env:APB_SCAN_ALLOW_UNVERIFIED_BUILD = '1'
python -u -B .\scripts\apb_capture_pumpkins.py 6376 .\captures\apb-level-20261018-streamed-e9c009d1\profile.json .\captures\pumpkin-nearby-new --radius 8000
```

Use the actual process ID and an unused output directory. This exact-build survey follows WorldInfo +0x44 to the persistent level, then its +0x44 owner and the owner's +0xEC level array. Every registry entry must retain the observed ULevel vtable; registry bounds and membership are reread. Level ownership is not assumed to be identical across registry entries. The live probe found 258 typed levels, whereas the earlier capture contained 13.

Only typed audio emitters with a +0x274 link to a typed prefab, consistent fingerprints and actor membership, and locations within the requested radius are retained. The saved profile opens in the existing F8/F6 annotation overlay. These profiles also apply the radius against the current camera each frame. Both members of each association have markers; the pattern does not prove pumpkin identity or collection availability. F8 reloads the saved capture; moving into a new area requires another capture.

`scan.jsonl`, `association-capture.json`, `profile.json`, and `annotations.jsonl` retain scope, raw associations, the selected profile, and operator annotations. Capture failures remain explicit and logged; no general-object profile is substituted.

Profiles require status `unverified_build_candidates`. Optional `object_name` values appear beside each address and position offset; annotations remain bound to exact instance and position offset.

Run from an elevated PowerShell with APB open:

```powershell
.\scripts\run_apb_object_annotation_overlay.ps1 -Profile C:\path\build-candidates.json -Output C:\path\pumpkins.jsonl
```

Only a click-through overlay appears. Focus APB, press F8, aim the centre reticle at an object candidate, and press F6 to tag it `pumpkin`. `-Tag` changes the tag. F5 cycles the on-screen candidates; F7 undoes the latest annotation; F8 stops or reloads the explicit profile on restart; F11 exits. No player ownership filtering is applied.

The profile must have format `apb-build-candidates-v1`, the exact current `Session.identity` under `identity`, `module_base`, `camera`, and `candidates`. Each object requires `address`, `vtable_rva`, `marker`, `position_offset`, and `instance_signature`. Camera additionally requires `matrix_offset`. Numeric fields accept integers or `0x` strings. Supply a new profile after process restart or district changes. F8 reloads this file; it does not initiate a heap scan.

Camera offsets and candidate meanings remain provisional until observed in the game. Coordinates project using the existing camera-relative matrix convention; a plausible matrix is not proof of camera correctness. The launcher explicitly enables discovery of unverified builds and logs their actual identities; it changes no production hash gate.

Every sample rechecks fingerprints, type markers, position bounds and camera matrix shape. Replaced candidate instances are removed from the active candidate list, their tags are invalidated, and each removal is logged with a visible expired-instance count. Camera changes and failed reads stop projection and record an error. Focus loss immediately hides all projections; focus return samples fresh memory. F8 reloads the explicit profile; obtain a fresh scan to discover replacement instances.

The append-only JSONL contains readiness, start/stop/error events, annotations and undo events. Each annotation records process identity, exact object fingerprint, type, position offset, raw body evidence, camera evidence and projection. The body extent is a bounded probe, not a verified allocation size. Tags apply only to the current process and exact instance; they are human observations, not an automatic pumpkin classifier. Existing output files are never overwritten.
