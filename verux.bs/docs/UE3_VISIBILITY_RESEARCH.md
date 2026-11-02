# UE3 visibility research

Research date: 2026-10-02. Scope: UE3/UDK mechanisms and their relevance to APB.

## Conclusion

The existing F8 observations do not establish an automatic visibility implementation. They record operator labels and raw contexts; no verified query, field, native bone mapping or obstruction result connects those observations to engine behavior.

The most defensible next investigation is APB's actual obstruction-query semantics. Rendering recency can be researched separately, but must not become a silent substitute for obstruction evidence. Public UE3 documentation establishes engine concepts, not APB offsets or the behavior of its customized engine.

## What visibility can mean

| Question | Required evidence | Insufficient evidence |
| --- | --- | --- |
| Is a point inside the view? | Camera transform and projection | Actor presence in a roster |
| Was a component rendered recently? | Verified component render timestamp and engine clock | A plausible changing float |
| Is a point unobstructed from the camera? | Query with documented origin, endpoint and blocking rules | Render recency |
| Can a shot reach a body region? | Weapon origin and matching collision/weapon rules | Camera LOS alone |
| Which body regions are clear? | Verified animated endpoints and individual obstruction results | Pawn origin or collision bounds |
| Would the server accept a hit? | Server gameplay rules and relevant authoritative state | Client geometry alone |

The last four rows are requirements inferred from the distinction between rendering and collision, not claims that APB exposes corresponding fields.

## Rendering visibility

Epic documents UE3 distance, frustum, precomputed and dynamic occlusion culling. Dynamic occlusion works per primitive component using GPU depth queries, with results read back a frame later. This is rendering evidence, not a point-specific obstruction query. See [Epic UE3 Visibility Culling](https://docs.unrealengine.com/udk/Three/VisibilityCulling.html).

UE3 precomputed visibility does not handle movable objects or movable occluders. It therefore cannot independently answer a moving character's limb visibility or changing vehicle obstruction. See [Epic UE3 Precomputed Visibility](https://docs.unrealengine.com/udk/Three/PrecomputedVisibility.html).

The UDK February 2013 Actor declaration defines `LastRenderTime` using `WorldInfo.TimeSeconds` and explicitly notes render-thread delay. PrimitiveComponent exposes a corresponding timestamp and a different `LastSubmitTime`. Source declarations are primary code evidence, hosted in a third-party mirror; they are not an APB SDK:

- [UDK Actor.uc](https://raw.githubusercontent.com/ChristopherLeslie/StockholmGame/master/UDK-2013-02/Development/Src/Engine/Classes/Actor.uc)
- [UDK PrimitiveComponent.uc](https://raw.githubusercontent.com/ChristopherLeslie/StockholmGame/master/UDK-2013-02/Development/Src/Engine/Classes/PrimitiveComponent.uc)

Consequences: identify the owning mesh and correct clock before interpreting recency; distinguish submission from rendering; measure latency rather than adopting a universal threshold. An actor timestamp cannot locate an exposed arm. Whether shadows or additional views update a particular APB timestamp remains unverified. Modern UE4/UE5 reports about those behaviors do not prove UE3/APB behavior.

## Obstruction queries

Epic's UE3 collision guide documents:

- `Trace`: returns the hit actor and can supply hit position, component, physical material and skeletal bone name.
- `FastTrace`: returns a boolean clear-path result; bullet behavior is optional.
- `TraceComponent`: checks only the supplied component, so it cannot alone establish world LOS.
- Zero-extent and nonzero-extent checks have different blocking rules. Bullet flags can change collision geometry selection.
- Unreal collision and PhysX are separate parts of collision response.

Thus query origin, endpoint, filters, extent and geometry selection are part of the result's meaning. A clear ray with incorrect filters proves little. See [Epic UE3 Collision Technical Guide](https://docs.unrealengine.com/udk/Three/CollisionTechnicalGuide.html).

The Actor source specifically describes `FastTrace` in terms of world geometry. Do not assume it includes every dynamic actor or vehicle. The Controller source declares native `LineOfSightTo`, with optional check location and alternate target location, and states that it does not account for FOV. The native implementation is not supplied by that declaration; exact sample points and APB behavior remain unresolved. See [UDK Controller.uc](https://raw.githubusercontent.com/ChristopherLeslie/StockholmGame/master/UDK-2013-02/Development/Src/Engine/Classes/Controller.uc).

Neither a native declaration nor a boolean return implies a persistent per-target field readable by an external tool. A query can calculate its answer on demand and retain no durable result. This is an architectural inference and an important limit of broad memory comparisons.

## Bones, sockets and partial exposure

UE3 sockets are named attachments offset from bones. Epic documents retrieving socket world transforms and the associated bone name. Socket and bone positions must not be conflated. See [Epic UE3 Skeletal Mesh Sockets](https://docs.unrealengine.com/udk/Three/SkeletalMeshSockets.html).

For APB, the missing prerequisites remain mesh ownership, actual skeleton identity, animated transform interpretation and world composition. A collision cylinder or box is insufficient. A ray to a joint also samples a point rather than the complete visible surface around that joint.

Proposed semantics, pending implementation evidence:

- Optical exposure: evaluated from the camera using optical obstruction rules.
- Shooting clearance: evaluated from the verified firing origin using the selected weapon's rules.
- Partial exposure: some evaluated body points clear and others blocked. State the sample set explicitly.
- Unknown: missing, stale or invalid identity, transform, query or filter evidence.

Transparent glass is a useful discriminator: it can allow optical exposure while preventing a shot. An opaque wall is a baseline, but cannot distinguish optical and weapon queries by itself. Camera and firing origin can also disagree near cover in third person. These are proposed diagnostic cases, not verified APB collision classifications.

## APB-specific evidence and limits

The repository contains these literal settings:

- `assets/src_ini/DefaultGame.ini:57`: `[APBGame.cAPBPawn]` / `m_fVisibilityChecksFrameTimeLimit=0.0f`.
- `assets/src_ini/DefaultGame.ini:373`: `[APBGame.cVehicleManager]` / `m_cfg_nMaxVisibilityChecksPerVehicle=8`.

They are leads for game-specific systems. Names and values do not establish check purpose, scheduling, trace origin, geometry, cached result or runtime use. In particular, zero could have several meanings; do not infer disabled, unlimited or instantaneous operation without implementation evidence. The vehicle setting could concern vehicle management rather than combat LOS.

APB's publisher confirms a 64-bit upgrade in version 1.30. Older engine-upgrade announcements describe other development branches, and do not establish the structure of the current executable. See [APB 1.30 release notes](https://www.gamersfirst.com/apb/news/2023/2/8/apb-130-patch-notes). Record executable hash/build for any later finding. Do not transplant 32-bit UDK offsets, native indices or layouts into APB.

## Recommended investigation order

1. Define whether the requested result is optical visibility, shooting clearance, or both. Use separate result meanings where they differ.
2. Establish the current APB build and trace the purpose of its visibility-related declarations through available authorized source, script metadata or diagnostic interfaces. Locate actual query producers before proposing a stored result.
3. Verify target skeletal ownership and animated endpoint positions independently. Check standing, crouching, turning and animation; repeat after respawn and with another mesh.
4. Establish query origin, blocking filters, dynamic-object handling and freshness. If accessible evidence only supports render recency, report that limitation explicitly.
5. Evaluate a specific candidate against open space, solid wall, partial cover, glass, moving vehicle/door, camera-versus-firing-origin disagreement and view exit/re-entry. Repeat transitions in both directions.
6. Retain provenance: build, target instance, mesh, endpoint, origin, query type, filters, time/frame, result and blocker where available. Missing blocker data must remain explicitly missing.

These are manual research/validation cases, not new automated tests. No new captures should be described as implementation proof until they discriminate a named hypothesis. Existing F8 evidence can support a focused hypothesis, but more labels alone do not supply a query mechanism.

## External read-only constraint

Reading a field is different from executing an engine function. If the existing external reader cannot execute the selected native query, that capability does not emerge from knowing its name. A cached result would need proof of its producer, target, origin and update timing. Reconstructing collision externally would be a separate substantial implementation requiring verified map geometry, transforms, dynamic blockers and matching filters, and must not be introduced as an automatic fallback.

## Research limitations

No public source reviewed establishes APB's current LOS offsets, a per-bone visibility array, a callable current-build function address or a reliable universal timestamp threshold. Several legacy Epic pages were unavailable, including the weapons technical guide; no conclusions depend on their unread contents. Native UE3 function declarations expose contracts, not full native implementations. This research changes no product behavior and claims no live APB verification.
