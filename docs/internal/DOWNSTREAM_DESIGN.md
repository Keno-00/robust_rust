# Internal downstream design contract

Source-internal only. Do not bundle this document in installers, catalogs or customer help. This governs future feature work by maintainers and AI agents; the live UI keeps its existing visual style and concise labels.

## One decision, three consumers

`ProfileConfig::eligibility()` defines available consumers from compile-time edition, Memory Features master, feature masters and selected sources. `effective_config()` derives the runnable view; preparation, ML and runtime use it. UI uses the same eligibility for visibility and enabled state. Never reconstruct contradictory capability checks in a new feature.

Saved choices are preferences, not authorization. Keep preferences when a parent becomes unavailable; visibly disable/hide the child and strip it from the effective runnable config. Never switch acquisition source, silently bypass a hash or licensing guard, or introduce an alternate implementation. Signed server entitlement and supported-build checks still apply when an eligible memory consumer actually starts.

## Current hierarchy

| Parent | Children | Parent unavailable |
|---|---|---|
| Compiled Memory capability + Memory Features | Memory aim/trigger, player ESP, radar, pumpkins/projectiles, driver actions | No memory initialization or APB hash requirement; children unavailable |
| Aim master + eligible source | Aim activation, targeting, tracking, aim humanization/ML, target indicators | Hide related controls; exclude from preparation/runtime |
| Color aim source | Color acquisition and Color ML recordings/models | Other-source models are not selectable |
| Memory aim source | Mission filters, distance priority, recency gate, approximate body anchors, Memory ML | Other-source controls unavailable |
| Trigger master + eligible source | Trigger-specific filters and activation | Hide children; no trigger worker |
| Independent screen visuals | Crosshair, FOV and screen-only elements | Do not tie these to memory merely because they share Visuals |
| Closest Point vertical mode | Head/Neck/Torso/Feet subset | Hide subset; reject empty subset at Start |

Current vs No Y remains available for Color. Approximate anchors require memory body bounds; these are not actual skeletal bones. Target priority selects a player; vertical tracking selects a point on that player. Never merge the two decisions.

## Procedure for agents adding or moving a feature

1. Inspect the selected implementation, current source evidence, existing catalog and all consumers before changing code.
2. Propose its category using this short card: user purpose; proposed tab/section; feature master; source; edition/entitlement; required signal; dependent controls; what happens on parent/source/World loss; profile migration; evidence confidence.
3. If an existing row unambiguously covers it, apply that row. If new placement changes meaning, introduces a category, combines sources or hides an independent feature, ask the user with a concrete proposed layout and one alternative. Continue independent authorized work while awaiting that answer. Do not invent the user's taxonomy or repeatedly ask about already approved rows.
4. Separate approval of investigation from approval of implementation/publication. A class name is a discovery lead, not proof of a field's meaning, lifecycle, ownership, playback or hit/death event.
5. Implement the eligibility once and connect UI, preparation and runtime. Account for disabled masters, source changes, saved incompatible selections, unavailable edition, target loss, district changes and server revocation.
6. Reuse current UI placement, widgets and spacing. Product display labels use the authenticated encrypted catalog; internal names/docs are not embedded customer explanations.
7. Document evidence and unresolved gaps here or in linked internal feature notes. Follow session testing permissions: do not add or run tests without user authorization. Report builds/typechecks separately from user live acceptance.

## Product boundary

One repository: Full includes Color and Memory; Color compiles without actual driver transport, offsets or scans. Compile capability does not grant a license. Preserve existing Full enrollment; Color uses its explicit product identity and separate server secrets, artifact/storage namespaces and signed product manifests. No UI-only premium unlock.

## Categories still under discussion

Weapon labels need verified pawn-to-equipped-weapon identity. Looking-at-you alerts need remote facing evidence, not the local camera. Hit/kill audio needs a trustworthy event and user-approved distinction between a local hit/kill and any player's death. WAV availability alone does not establish that event. Generic audio emitters are diagnostic until active playback semantics are confirmed.


## User decisions — 2026-10-19

- The memory capability checkbox belongs only in General and is labeled Memory. Monitor Resolution belongs in Computer Vision.
- Hit sounds must use a verified direct memory signal distinguishing red/flesh hits from white hits. The user explicitly rejected pixel detection. Captured hitmarker images are diagnostic ground truth only; never introduce them as an automatic runtime fallback.
- Initial remote-facing and nearby-player identity probes did not establish a looking-at-you address, display name, equipped model or ammo count. Keep those unresolved; do not promote a 180-degree visual mismatch into a simple sign correction without rotation evidence.
