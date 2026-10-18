# APB core update — October 18, 2026

User requested updating Verux APB from the captured core offsets. Normal builds now require executable SHA-256 `35AEF01734AAB20B4B0C622ADDF5706F9E75CAAA98D5B84A0EA14D9DF72EE453`. The previous executable is no longer accepted by the normal hash gate. The separately authorized `apb-hash-test` feature remains explicit and is not used for this release build.

## Applied constants

| Definition | New value |
|---|---|
| Camera table RVA | `0x3DCE240` |
| HUD vtable / name index | `0x2B67088` / 16929 |
| WorldInfo vtable / name index | `0x316FF28` / 430 |
| Controller vtable / name index | `0x2A87258` / 998 |
| Pawn vtable / name index | `0x2AB7090` / 5778 |
| Vehicle vtable / name index | `0x2B320A8` / 6064 |
| Collision component vtable / name index | `0x31A54F8` / 3269 |
| Cursor pointer RVA / integer field | `0x3BD3340` / `+0x134` |

Camera matrix `+0x680`, camera position `+0x6C0`, pawn position `+0x88`, ownership links, roster fields, collision geometry fields, mission fields and render timestamps retain their existing offsets. Existing exact-type, fingerprint, ownership, roster, foreground and process checks remain in effect through the same EchoDriver reader. No alternate source is introduced. Diagnostic discovery scripts retain their historical hash/identity convention so prior evidence stays reproducible; their explicit override does not affect the native release gate.

## Evidence and limits

Source evidence is listed in [build discovery and runtime audit](APB_BUILD_CANDIDATES_20261018.md). The user reported pawn and ownership behavior as fine during the overlay session. Projection, positions and collision geometry have structural evidence and interactive observations. This does not establish all restart, district, passenger and respawn cases.

Cursor reads are `pointer = read_u64(APB_base + 0x3BD3340)`, then `value = read_u32(pointer + 0x134)`. White 130 and green 48 repeated in eight operator-labelled captures. Red 99 is the retained prior-build mapping, not directly confirmed on this build; matching the other colors does not prove red. The existing red mapping remains available for the compatibility update, with that limitation explicit. Unexpected values still stop the cursor feature through its existing diagnostic path.

Mission token `0xFFFFFFFF` was confirmed outside a mission. Active teammate/opponent classification retains its existing logic but has not been validated in a mission on this build. No new relationship inference was added.

Render timestamps remain a recency filter only. The user observed that moving players behind walls can remain recent while stationary hidden players become stale. This disproves using recency as wall visibility, beyond merely a short grace period. The update does not turn it into a geometry trace or change its threshold.

## Additional feature readiness

| Feature | Current scope |
|---|---|
| Manually confirmed pumpkin markers | Can implement exact-instance labels for the tagged locations; availability must remain unknown. Emitter/prefab matching alone also finds ammo distributors. |
| Generic projectile markers | Can implement a diagnostic live marker and first/last observation time for typed/name-matched projectile actors. Discovery must poll automatically; fuse, exact spawn/detonation time and exhaustive projectile coverage remain unverified. |
| Nearby component/object annotations | Existing annotation overlay supports explicit labels and bounded raw evidence. |
| Pumpkin availability / automatic pumpkin-only classification | Needs a discriminating gameplay actor or component and a recorded destruction transition; persistent emitter/prefab bytes did not change. |
| True wall visibility | Needs independent obstruction evidence; rejected recency heuristic is insufficient. |
| Grenade fuse or predicted blast timing | No verified timing field. Observation age must not be labelled fuse time. |
| Health/stamina bars | No accepted resource offsets; previous candidates were geometry/pose fields. |

## Build

```powershell
cargo build --locked --release --bin verux_apb --target-dir target/v3-release
```

Expected executable: `target/v3-release/release/verux_apb.exe`. No automated tests were added or run for this constant update. Build results and native live acceptance are reported separately; discovery overlays are not a native client run.

Release build completed successfully in 5m 44s on October 18 at approximately 13:17 Manila time, without `apb-hash-test`. Artifact size: 14,241,792 bytes. Artifact SHA-256: `C52BCD68111627094882D21063F804EBB5D4B41342D0EDD464AD595004BB49DF`. The native client was not launched or exercised against APB as part of this update; red, active mission classification and restart/district behavior remain pending live acceptance.
