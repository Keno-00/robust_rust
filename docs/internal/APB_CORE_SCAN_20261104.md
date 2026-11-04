# APB core scan — November 4, 2026

Requested scope: fresh F8-paced core discovery using EchoDriver against the
running APB session. Operator confirmed Financial district, on foot. Mission
state was not supplied. No product constants, customer UI or release changed.

## Identity and collection

- APB PID `1452`, creation FILETIME `134382696964582094`.
- Executable SHA-256 `35AEF01734AAB20B4B0C622ADDF5706F9E75CAAA98D5B84A0EA14D9DF72EE453`.
  This matches the October 18 supported product build. The diagnostic's older
  September hash gate still required its explicit, logged discovery override.
- Module base `0x140000000`. All process-memory bytes came from
  `Session.read` through encrypted EchoDriver v3. No alternate reader was used.
- The installed EchoDriver service was stopped. The elevated helper started it,
  successfully read the APB module header, then waited for foreground F8.
  No driver installation, service startup-type change or key provisioning occurred.
- Discovery completed at 20:52:57 Manila time. Raw source:
  `captures/apb-core-20261104-1452/candidates.json`.

Discovery retained 1,996 array hypotheses, 160,949 typed object prefixes, one
structural camera candidate and 7,243 nearby position candidates. It encountered
266 array-read failures, 5,607 object-read failures, six changing array headers
and three truncated arrays.
These are bounded discovery results, not exhaustive coverage. Exact array errors,
bounds, hashes and object-read errors remain in the archived report.

## Reobtained core

The camera candidate was `0x4DB9600`, referenced by table RVA `0x3DCE240`, slot 1.
Its vtable RVA/name index were `0x2B67088` / 16929. Its fingerprint remained stable
through discovery. The short audit at 20:54:20 rechecked the current module root
and obtained six passing structural snapshots:

| Check | Observation |
|---|---|
| HUD/projection binding | Same camera, slot 1; unique typed HUD, matrix shape and guarded binding passed. |
| Local ownership | On foot; World `0x94F4510`, controller `0x7CA7C080`, pawn `0xA600C100`; reciprocal ownership and position checks passed. |
| Rooted roster | World `+0x48C`, next `+0x350`; two pawn nodes and 11 vehicle nodes, including the local pawn. These are roster nodes, not active-player counts. |
| Collision bounds | Component `0x196C79D00`, vtable RVA/name index `0x31A54F8` / 3269; extents `(34,34,91)`, half-height 91, radius 34; ownership and reread passed. |
| Render timestamps | World clock advanced and pawn timestamps met consistency checks. This does not establish obstruction or visibility. |
| Mission raw layout | Stable raw blocks at pawn `+0x990`; local token `0xFFFFFFFF`. No active-mission or team semantics inferred. |

These roots/types agree with the existing core definitions. This session provides
fresh launch-bound structural evidence; it does not justify an offset or hash update.
Cursor colors, hitmarker events, true visibility, vehicle/passenger transitions,
death/respawn, district travel and restart reacquisition were not validated here.
No screenshots or visual projection acceptance were obtained for this scan.

Subsequent same-launch cursor, hit-event and vehicle entry/exit results are in
[the follow-up confirmation record](APB_CORE_CONFIRMATION_20261104.md), including
the user's decision to leave district-change and restart live checks pending.

## Audit correction and validation

The first audit failed with `HUD binding changed during audit` and skipped its
dependent checks. It compared all 16 root bytes, including adjacent counters,
and always returned exit zero after per-check failures. Its report did not retain
the before/after root bytes, so the exact triggering difference is unknown.

The audit now reuses `ValidationRuntime.camera` / `guard_hud`, the existing live
implementation documented in the October 18 investigation. It validates active
entry bounds, table identity, selected slot, unique typed HUD, fingerprint and
projection shape across rereads, and retains its diagnostics. Failed checks now
produce a failing report and nonzero process exit. The separate `-r2` audit passed
all six checks. The failed original report is preserved alongside it.

Python syntax/import/CLI-help checks, PowerShell parsing and `git diff --check`
passed. The elevated F8 helper and corrected audit were exercised against APB.
No automated tests, test files or dependencies were added; no release build ran.

Reproduce discovery from an elevated shell in the main checkout:

```powershell
.\scripts\run_apb_core_f8.ps1 -OutputDirectory captures/apb-core-NEW
# After F8 and CAPTURE COMPLETE, still in that exact process launch:
$env:APB_SCAN_ALLOW_UNVERIFIED_BUILD = '1'
python -u -B scripts/apb_runtime_audit.py 1452 captures/apb-core-NEW/candidates.json captures/apb-core-audit-NEW
Remove-Item Env:APB_SCAN_ALLOW_UNVERIFIED_BUILD
```

Replace PID 1452 with the current APB PID. Reuse no old output directories.
The audit remains explicitly gated to its supported executable hash.

## Evidence and handover

Complete new raw discovery, operator context, both audit reports and relevant
launch logs are committed in
[`evidence/apb-core-20261104-1452.zip`](evidence/apb-core-20261104-1452.zip).
The adjacent [JSON manifest](evidence/apb-core-20261104-1452.json) records archive
SHA-256, CRC validation, each source file hash and a compact findings summary.
Original working copies remain under `captures/`; preexisting captures and
`.wrangler/` were untouched. No files were intentionally created outside the repo.
Nothing was deployed or published. EchoDriver remains running.

The user also directed a later Pumpkins decommission: an explicit reversible
decommissioned flag, hidden UI and safely disabled behavior, no delivery, and a
notice-board entry later. That product work is deferred because the user made
the F8 scan the sole current focus. It has not been implemented or published.
