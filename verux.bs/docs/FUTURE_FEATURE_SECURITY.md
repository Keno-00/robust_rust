# Future feature security workflow

Status: engineering workflow with a 2.0 source baseline. Updated 2026-10-03.

Use with [the security implementation plan](SECURITY_IMPLEMENTATION_PLAN.md) and repository `AGENTS.md`. Use [the protocol and rollout procedure](SECURITY_PROTOCOL_V2.md) for implemented boundaries and explicitly pending gates; source implementation does not mean production deployment.

## Before implementing a feature

Create or update a focused document under `docs` describing:

1. User-visible behavior and compatibility with existing settings.
2. Assets to protect: algorithms, model weights, entitlement, user data, credentials, or operational capacity.
3. Trust boundaries and attacker-controlled inputs, including client memory and requests.
4. Execution location: local host, authorized in-memory algorithm, or server-only computation. State why timing and secrecy requirements allow that choice.
5. Data collected, transmitted, stored, logged, and deleted. Do not transmit captured application data merely to authorize an algorithm.
6. Numerical/input constraints, maximum resource consumption, errors, cancellation, and cleanup.
7. Versioning, migrations, release order, rollback, and reproducible validation evidence.

If implementation secrecy is mandatory, the algorithm cannot be delivered to the client. If local execution is mandatory, explicitly accept extractability and describe cost-raising measures without claiming prevention.

## Algorithm feature contract

Every delivered algorithm needs an explicit product-scoped identifier, immutable version, supported ABI/runtime versions, typed input/output schema, finite-number and range rules, memory/time limits, and state-reset semantics. Document which settings select it and whether changing those settings requires reload.

Use narrow numerical interfaces. Algorithms cannot request arbitrary host functions, file paths, network destinations, process access, or executable loading. Reject missing/unsupported artifacts visibly. Do not keep a secret local copy as a backup.

For each feature document, include this record:

```text
Feature / product:
Algorithm ID / version / ABI:
Owner:
Protected assets and remaining extraction risk:
Local inputs and server-transmitted data:
Settings and compatibility:
Authorization / manifest verification:
Input and output bounds:
Execution and resource budgets:
Pause / stop / cancellation / authorization-loss behavior:
Sensitive buffers and cleanup ownership:
Failure dialog and sanitized diagnostic codes:
Artifact retirement / key rotation / rollout / rollback:
Validation commands, manual cases, results, and unresolved risks:
```

## Server and storage rules

The authoritative Verux server stays in this repository's `license-server`. New products enroll using explicit product identities, product-scoped hashes, storage, entitlement rules, and official server-only Payhip secrets. Never fabricate absent credentials or copy the license authority into product repositories.

Validate schema, request size, authorization, product scope, versions, and replay state at every endpoint. Bind query values; allowlist identifiers. Use independent abuse limits that cannot all be reset by changing client-supplied strings. Serialize entitlement and usage changes and document crash recovery.

Treat all local configuration/model data as untrusted. Encryption at rest is not authorization. Distinguish intentional editable user settings from signed vendor artifacts. Never place long-lived shared secrets or signing private keys in the client. DPAPI does not establish trust against the same Windows user.

## UI and diagnostics rules

Startup preparation displays only a 0-100% bar and percentage. Do not expose internal stage names or algorithms through routine UI text. Completion means all required preparation succeeded. On pause/stop, release algorithm execution resources before reporting teardown complete.

Failures remain visible and actionable with sanitized diagnostic references. Log enough to diagnose selected-implementation failures, but never credentials, decrypted payloads, sensitive captured data, or raw upstream responses. Hidden status details do not replace server validation or signatures.

## Review and release gates

- Confirm feature docs describe final behavior, not abandoned designs.
- Confirm entitlement and product checks are server-owned; acknowledge that local checks are patchable.
- Verify payload integrity, compatibility, bounded execution, reference ownership, and cancellation races.
- Run the applicable commands in the implementation plan and manual lifecycle/performance checks. Capture results and report failures.
- Check dependency changes, licenses, secret handling, logging, and release artifact signing.
- Document migration and rollout ordering so existing enrolled products continue to work.
- Include an explicit residual-risk statement and operational recovery procedure.

Do not add new automated tests, test files, fixtures, or test-only dependencies without explicit user request. Do not weaken validation to pass checks. Do not add automatic fallbacks. When a selected implementation cannot work, stop visibly with actionable diagnostics.

## Documentation maintenance

Update the feature document, protocol/version matrix, artifact inventory, and deployment notes in the same change as code. Add links from the relevant module/product documentation. Record security decisions with date, rationale, rejected alternatives where relevant, and validation evidence. Keep secrets and exploit-enabling customer information out of documentation.

Periodically review active artifacts, accepted client/runtime versions, public verification keys, dependencies, audit retention, and backup recovery. After an incident, record affected versions, containment, rotations, recovery, and corrective work. Never describe planned protection as already deployed.
