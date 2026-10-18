# Captured names for feature discussion

Source internal. This is an evidence index, not approval to implement additional features.

## Scope and confidence

Read-only review of saved captures on 2026-10-18. No new live scan, process read, or automated test was performed. Main source: `captures/apb-level-20261018-streamed-e9c009d1/level-capture.json`, PID 6376, APB SHA-256 `35AEF01734AAB20B4B0C622ADDF5706F9E75CAAA98D5B84A0EA14D9DF72EE453`.

The capture contains decoded name strings, typed object headers, and bounded 768-byte actor bodies. Some unrelated decoded strings are corrupt; the exact readable strings below are discussion leads. A readable class name plus an object header does not establish any field's gameplay meaning or current ownership. Old addresses must not be reused in a restarted process.

## Weapons

| Exact captured name | Name index | Observations | What remains unverified |
| --- | --- | --- | --- |
| `cRaycastWeapon` | 6780 | Two instances: `0x13A677580`, `0x13A676080`; vtable RVA `0x2B6AC58` | Weapon model/name, selected slot, owner, ammunition, firing state |
| `cGrenadeWeapon` | 6778 | Instance `0x13A674B80`; vtable RVA `0x2B6A120` | Selected grenade type, inventory count, held state, fuse |
| `cWitnessingWeapon` | 6782 | Instance `0x13C9E7580`; vtable RVA `0x2B6B858` | Meaning and relationship to witnessing/arrest mechanics |
| `cWeaponAttachment` | 6766 | Two instances with repeated pointers to distinct captured pawns | Whether attachment is currently equipped, holstered, or a rendered accessory |
| `cWeaponCustomisationInfo` | 6591 | Multiple captured instances | Association to equipped weapon and user-visible model/customization labels |

The saved bodies provide an actual association lead: attachment `0x15A6ADC00` points to pawn `0x11D6A10C0` at both `+0x1A0` and `+0x1EC`; attachment `0x1601AD800` points to pawn `0x23AABA080` at the same two offsets. Each pawn also points to itself at `+0x1A0`. These are static pointer observations, not validated ownership field definitions. An equipped-weapon feature needs controlled weapon switching and owner/model validation.

## Additional exact names

| Exact captured name | Name index | Discussion lead; not confirmed behavior |
| --- | --- | --- |
| `cRemoteHUDMarker` | 6685 | Existing game marker relationships or labels |
| `cClientHUDMarker` | 6683 | Local game marker relationships or labels |
| `cReplicatedHUDMarkerGroup` | 6689 | Grouping and replication of game markers |
| `cDisplayPointManager` | 6792 | Display points and their gameplay meaning |
| `cHoldableItemManager` | 6567 | Carried-item relationships |
| `cStorageInventory` | 6599 | Inventory objects and their scope |
| `cVehicleCargoInventory` | 16840 | Vehicle cargo relationships |
| `cVehicleRepairTrigger` | 6515 | Repair area classification; active state not known |
| `cOpenWorldDropOff` | 17057 | Drop-off locations and their activation/mission eligibility |
| `cContact` | 6615 | Contact locations or identity |
| `cCheckpoint` | 6555 | Checkpoint locations; active state not known |
| `cPlayerVehicleSpawnZone` | 17063 | Spawn-zone positions and boundaries |
| `cPlayerVehicleSpawnDirection` | 6963 | Spawn-direction objects |
| `cParkedVehicleSpawner` | 6068 | Parked vehicle potential locations, not proof of a spawned vehicle |
| `cAnimationDrivenTrigger` | 6961 | Animation-driven interaction trigger; meaning unknown |
| `cAPBGameReplicationInfo` | 6629 | Potential match/mission state container; fields unverified |
| `cAPBTransientVFXManager` | 17037 | Transient visual effect management, not an established hit event |
| `cPooledAPBEmitter` | 6708 | Pooled effects, not proof of audio playback or damage |

The user's game knowledge should guide which of these are meaningful. New investigations should be scoped to one known gameplay event and labelled before/after evidence, rather than equating a name with a feature.

## Looking-at-player notification

`cAPBCamera` (6718) was captured once at `0xF33450C0`, vtable RVA `0x2B632C0`. `CameraActor` (3332) was also captured. Existing verified camera projection is local. No evidence here establishes a separate camera or trustworthy aim rotation for each remote player. Pawn-facing direction, camera direction, and weapon aim can differ. A beam or “looking at you” notification would need remote orientation semantics, ownership, axis conventions, and an angular/distance acceptance check. It cannot currently be described as ready without additional evidence.

## Hit and death sounds

The reviewed readable name set does not establish a local successful-hit event, death event, or damage attribution. `cHUDBase`, the replication object, and transient emitter names are leads, not event proof. A WAV asset can supply sound playback once an event is confirmed, but asset availability does not establish detection. Triggering from visible hitmarker pixels would be a distinct screen-based design requiring user agreement and false-positive handling; it must not be introduced as an automatic fallback for a memory event reader.

## Audio emitter activation

`cAudioEmitterPositioningActor` (16985) was captured at many locations, including pumpkin and non-pumpkin locations. Its presence/position cannot tell us whether sound is playing. Saved tagged-state evidence (`captures/pumpkin-tagged-state-audit.json`) contains emitters at a shot-after-tagging location and already-smashed location. Earlier bounded prefab/emitter bytes did not provide a validated activation transition. No sound-cue identity, play/stop flag, attenuation rule, trigger radius, schedule, or playback callback has been confirmed.

The later labelled destruction recorder (`captures/pumpkin-visible12-empty29-shot-summary.json`) validates paired `cProp` hidden masks for pumpkin availability, not emitter playback. The visible prop changed masks after destruction while an empty-location control remained hidden throughout. Therefore there is presently no supported answer to exactly when the audio emitters activate; a controlled audible/not-audible capture is required to answer that question.

## Creative feature categorization (discussion only)

- Equipped weapon: Memory > Player information. Establish pawn -> current weapon -> model identity using attachment leads plus a controlled weapon switch. A weapon class cannot distinguish exact guns.
- Looking-at-you: Memory > Awareness. Reuse radar anchors and an Above/Below relative placement. Use the existing small sans-serif font, stable angular eligibility and cooldown; a beam can be optional diagnostic display. Remote orientation and player display-name linkage are not verified. Body-facing is not proof of aiming or line of sight.
- Hit sounds: Screen feedback > Hitmarker. The user proposed visible crosshair detection, a screen implementation requiring labelled on/off samples and edge/debounce. A held hitmarker must not repeat sound every frame. Memory damage detection is a distinct implementation, never a silent fallback.
- Death/kill sounds: Feedback > Confirmed event. Ask whether the scope is the user's own confirmed kills, visible deaths or all district deaths. Actor disappearance can be unloading; it is not sufficient death evidence. Keep kill and hit events separate.
- Sound selection: supplied assets/wav provides choices; enabled/sound/volume/preview controls belong under the approved event feature. Do not play unsolicited previews. Asset presence does not establish a detector.
