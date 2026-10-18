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

## Testing

- Do not add new automated tests, test files, fixtures, or test-only dependencies unless the user explicitly requests them.
- Existing tests may be run when useful for diagnostics, but test results are secondary to the requested implementation and should not drive unrelated changes.

### UI

- Do not, in any circumstance, disobey the current UI style.
- Do not add unnecessary expository text. 