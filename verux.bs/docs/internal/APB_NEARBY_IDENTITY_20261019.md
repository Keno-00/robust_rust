# Bounded nearby identity diagnostic — 2026-10-19 Manila

Internal evidence only; no production feature or accepted name offset was added.

## Scope and guards

- APB PID 20620; exact audited executable SHA-256 `35AEF01734AAB20B4B0C622ADDF5706F9E75CAAA98D5B84A0EA14D9DF72EE453`.
- Existing authenticated EchoDriver Session and ValidationRuntime only; current process identity, unique HUD, World, local possession, rooted roster and selected instance fingerprints checked before/after.
- Maximum three remote pawns, each bounded to `0x1400` bytes; at most eight directly linked bodies, each bounded to `0x800` bytes; bounded pointer-header probes. No whole-process scan, writes, game input or foreground changes.
- GNames decoding accepts packed, unaligned entries and verifies flags/index and current selected entry pointer. Object names do not by themselves prove class identity, ownership or equipped state.

## Findings

`captures/nearby-identity-20620-20261019/report.json` captured one remote pawn, `0x2111B7040`, at approximately 25.36 m. The second capture, `captures/nearby-identity-20620-20261019-r2/report.json`, saw that same instance at approximately 5.35 m and another remote pawn at approximately 86.56 m.

Both had a direct `pawn + 0x980` pointer to an object with vtable RVA `0x2B1EFD0` and decoded object name `cWeaponAttachment`. The nearby instance pointed to `0x174E7AC00`; its bounded linked body was saved. This confirms an observed pointer association. It does **not** identify the currently equipped weapon model, weapon ownership, ammo or weapon changes.

The pawn also referenced an object named `cAudioCharacterInterface` at `+0x988` (header evidence only). No activation, audibility or gameplay signal was established.

No reliable player name was found. A two-character UTF-16 candidate from the distant pawn was considered a probable false positive and is excluded from conclusions. No username should be rendered from these results.

The final attempt to resolve class identity through UObject `+0x30` stopped at the foreground guard after 15 seconds; **no additional pawn/link probe ran**. Its diagnostic is `captures/nearby-identity-20620-20261019-r3-class/error.json`. Scanning was not expanded or retried further.

Helper source: `captures/nearby_identity_20620.py`. The helper is a diagnostic capture utility, not a test or production implementation. Its latest version includes the additional class-pointer guard; earlier reports preserve their respective original evidence and limitations.
