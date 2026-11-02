# Repository Instructions

## Anti-fallback rule

- Do not add silent or automatic fallback implementations for capture, licensing, storage, input, rendering, networking, or other essential product behavior.
- Troubleshoot and correct the selected implementation until it is reliable. If it cannot operate, fail visibly with an actionable message and diagnostic log entry.
- A fallback may be added only when the user explicitly approves that exact fallback and its activation is visible in the UI and logs.

## Verux server ownership

- The authoritative Verux license server lives in this repository under `license-server` and must not be duplicated into product repositories.
- Product enrollment must use explicit product identities, product-scoped hashes and storage, and server-only Payhip secrets.
- Preserve existing enrolled-product behavior while adding new products; never invent or commit missing product secrets.
- Other Verux Products should ABSOLUTELY follow the security implementation verbatim. IF and ONLY IF, the current verux.apb cannot stand in other verux products, only then can the implementation be slightly modified. HOWEVER, DIRECTLY COPYING AND ADHERRING TO SECURITY FEATURES BOTH HOLISTICALLY and ATTENTIVELY is an ABSOLUTE MUST WITH NO REASON TO SKIP.

## Auditability

- Keep significant changes reviewable through focused modules, explicit product constants, migrations, documentation, and reproducible validation commands.
- Report validation failures directly. Do not hide them by switching implementations or weakening checks.

## Secrets and operator transparency

- Keys, credentials, signing material and private backups live only in the operator secrets folder, `C:\Users\Admin\Desktop\deploy\Verux Secrets` (resolved by `license-server/scripts/VeruxSecrets.ps1`). Never write them to `%LOCALAPPDATA%`, `%APPDATA%`, temp folders, or anywhere an app sandbox can redirect.
- Before creating, rotating, moving, uploading or deleting any secret, stop and ask the user. Afterward, update `docs/OPERATOR_SECRETS.md` in the same change.
- Anything that customers depend on or that is hard to undo needs the user's explicit go-ahead first, and a plain-language report afterward: deployments, migrations, catalog or release publication, trust anchors and pinned keys, scheduled jobs and Workers. Never leave it only in a doc.
- If an operation needs a secret you cannot find, stop and ask. Do not generate a replacement.
- Commit work that produced something shipped or deployed before ending the session, so the repository matches what users run.

## Testing

- Do not add new automated tests, test files, fixtures, or test-only dependencies unless the user explicitly requests them.
- Existing tests may be run when useful for diagnostics, but test results are secondary to the requested implementation and should not drive unrelated changes.

### UI

- Do not, in any circumstance, disobey the current UI style.
- Do not add unnecessary expository text.
## Downstream feature design (internal)

- Before changing feature eligibility, source behavior, category placement or edition availability, read `docs/internal/DOWNSTREAM_DESIGN.md` and follow its single eligibility contract and user-deliberation procedure.
- That document and other `docs/internal` materials are source-internal; do not package or embed them in customer flows.
