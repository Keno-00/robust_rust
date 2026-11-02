# APB cursor state via EchoDriver

Current native build update (October 18): pointer RVA `0x3BD3340`, then unsigned
32-bit value at `+0x134`. White 130 and green 48 were confirmed again; red 99
is retained from the previous build and still needs a new-build red capture.
The September evidence and commands below describe the previous executable.
See [core update](APB_CORE_UPDATE_20261018.md) for the current hash gate.

The preserved F8 offset-finder workflow and the separate camera/witness scan
entry points are indexed in [APB_F8_SCANS.md](APB_F8_SCANS.md).

This note records read-only discovery for the verified APB.exe SHA-256
`3BECA02821F50A7B8FEF3E16923FA3FCA0D0B0DD49CC9622520EA500001E1640`.
When Memory is selected as the triggerbot source, the product reads the cursor
through EchoDriver. The values and pointer path below were checked in two game
launches. Color Detection remains an explicit, separate triggerbot choice.

## Verified in two APB launches (2026-09-29)

EchoDriver snapshots of PID 4788 isolated the four-byte field at
`0x90A618B4`. The player confirmed a live terminal readout against what they
saw in game:

| Raw value | Observed cursor |
| --- | --- |
| `130` (`0x82`) | White, including during the tested sprint |
| `48` (`0x30`) | Green over the player's car |
| `99` (`0x63`) | Red over another character |

The earlier reported values `119`, `44`, `88`, and `0` did not reproduce in
this build. No value for a genuinely hidden cursor is established. One rapid
read returned `0xFFFFFFFF` while the field changed; its meaning is unknown.

The saved full writable-page snapshot yielded a direct pointer from
`APB.exe + 0x3BC4230` to `0x90A61780`, with the field at object offset
`0x134`. A live EchoDriver reader followed the pointer and returned the same
address and color values. One line reported a path mismatch because two
consecutive reads of the same field straddled a value change (`130` then
`0xFFFFFFFF`); the pointer target address itself still matched. The diagnostic
now reports this as a value change between reads.

APB's loader list located `APB.exe` at `0x140000000` (size `0x531B000`)
and `EOSSDK-Win64-Shipping.dll` at `0x7FFB0B870000` (size `0x1335000`).
No pointer directly to the cursor object was found in EOSSDK's range in the
saved writable-page snapshot. No EOSSDK pointer to the APB static pointer
was found either. This does not rule out other EOSSDK pointer chains or
references in unreadable/nonwritable pages.

After restarting APB as PID 7612, the APB.exe pointer was `0` while the game
loaded. In the district it became `0x8E4ED780`, and the field at offset
`0x134` again returned white `130`, green `48`, and red `99`. The live reader
reported no EchoDriver read errors. In another launch (PID 4208), the pointer
was `0` while APB was paused, then became `0x8DF59780` after the player
unpaused. The live readout showed white `130` and green `48`, and the player
also confirmed red. The product now waits without firing while this pointer is
null. It logs when waiting begins and when the pointer becomes ready. A driver
read failure or invalid nonzero pointer stops it with a visible runtime error.
The Triggerbot tab chooses whether Memory mode fires on red (`99`), green
(`48`), or both. White (`130`) never fires. A selected state can fire repeatedly
at the configured delay, and leaving that state clears its pending shot.

## Reproducible diagnostics

### Interaction preference for future scans

Let the player change game state at their own pace. Keep one elevated scan
terminal open, print the state currently needed, and wait indefinitely for an
in-game F8 press before capturing. Print when each capture has finished so the
player knows it is safe to move on. Do not use countdowns, short capture windows,
or repeated chat confirmations to coordinate white/green/red, sprint, spread,
or later pointer hunts. For validation, show the live raw value and interpreted
state in the terminal while the player moves freely and reports any mismatch.

The F8-paced discovery scripts are `scripts/apb_cursor_unknown.py` and
`scripts/apb_cursor_recheck.py`, launched by their matching PowerShell
wrappers. They bind each capture to the APB process creation time and verified
executable hash; candidate files cannot be reused across launches. The saved
pointer scan was produced with `scripts/apb_cursor_pointer_refs.py`, and
`scripts/apb_cursor_modules.py` inventories the target loader list through
EchoDriver because Windows module enumeration was denied.

For a live readout of the APB.exe pointer path, run an elevated PowerShell:

```powershell
powershell -NoProfile -File scripts/run_apb_cursor_live.ps1 -Leads '..\drayebr\build-rpm\snapshots\cursor-recheck-4788-20260929.json' -PointerRva 0x3BC4230 -FieldOffset 0x134
```

After a game restart, add `-AllowNewSession`. The prior absolute address will
not be read, and the terminal must be checked against white, green, and red
again. An invalid pointer, unknown value, or EchoDriver error must be reported
and must not silently select screen capture as a substitute.

The suggested spread float remains unverified. It must be discovered and
validated separately before any firing gate uses it.
