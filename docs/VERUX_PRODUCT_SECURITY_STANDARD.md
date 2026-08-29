# Verux Product Security and Release Standard

INTERNAL - DO NOT PUBLISH

## Immediate finding

The current `makumaku` and `tutokbot` Python sources verify Payhip licenses directly and contain reversible product-secret material. Obfuscation and one-file packaging do not make a client-held secret safe. Neither existing build should be treated as ready for public redistribution until licensing is migrated.

## Required shared standard

1. All Payhip and vendor credentials remain server-side.
2. Clients communicate only with the neutral Verux service over HTTPS.
3. License keys, installation identities, and session tokens are never logged or included in public notifications.
4. License and installation identifiers are stored server-side only as keyed hashes.
5. Each license permits one active, expiring session unless a product policy explicitly says otherwise.
6. Heartbeats use monotonic sequence numbers, short session lifetimes, replay rejection, and rate limiting.
7. The client stops protected runtime behavior when online authorization cannot be maintained.
8. Local state uses Windows DPAPI or an equivalent operating-system credential facility.
9. Every release has a version, SHA-256 digest, reproducible build instructions, and an archived source revision.
10. Public executables should be Authenticode-signed before broad distribution.
11. Updates must use HTTPS plus a signed manifest and binary hash. A downloaded executable is never launched until verification succeeds.
12. Logs rotate, redact sensitive values, and expose a documented troubleshooting location.
13. Fatal startup failures are visible to the user and also written to diagnostics.
14. Customer-facing announcements contain no administrator identifiers, secrets, internal endpoints, database identifiers, webhook details, or security-control thresholds.

## Product migration order

### 1. Verux APB

- Server-authoritative online licensing is implemented.
- One active session, heartbeat enforcement, rate limiting, and audit visibility are implemented.
- Release logging, fatal-error display, optimized linking, and final executable naming are implemented for Version 1.5.
- A secure automatic updater still requires a trusted binary host and release-signing key. Do not ship an updater that trusts only a downloadable SHA-256 value; the manifest itself must be signed.

### 2. Makumaku

- Remove the embedded Payhip-verification path completely.
- Assign a unique server-side product identifier and policy.
- Port the activation/session/heartbeat protocol used by APB.
- Move plaintext license storage to DPAPI-backed storage.
- Replace the existing release only after a clean build and secret scan.

### 3. Tutokbot

- Preserve the current dirty worktree before migration.
- Remove the embedded Payhip-verification path completely.
- Assign a unique server-side product identifier and policy.
- Port the APB activation/session/heartbeat protocol.
- Review model-file integrity and package size separately from license security.

### 4. Makulite

Do not rebuild from memory. First recover screenshots, binaries, configuration examples, manuals, supported hardware, and a written feature boundary. Treat any old executable as behavioral reference only and never extract or reuse embedded credentials. A new source repository should be created only after that specification is approved.

## Python versus Rust

Python is not automatically insecure, and Rust is not automatically uncrackable. Python one-file packages are generally easier to unpack and inspect, so no credential or authoritative decision can safely live inside them. Migrating sensitive decisions to the server is more important than changing languages. Rust may still be preferred for smaller native releases, performance-sensitive paths, and a shared updater/licensing library.

## Secure one-file update design

The distributed product can remain one executable. During an update, it may temporarily download the new executable beside the current file or into the user's temporary directory. The running version verifies a vendor-signed manifest and the binary hash, then launches the verified new version in replacement mode. The new process waits for the old process to exit, replaces it, relaunches from the original path, and removes the temporary copy.

Required infrastructure:

- A neutral HTTPS binary-download location.
- An offline-protected Ed25519 release-signing private key.
- The corresponding public key embedded in every client.
- A canonical signed manifest containing product, version, URL, size, SHA-256, and publication time.
- Rollback protection and a minimum-supported-version policy.

Do not enable automatic replacement until code signing and the update-signing workflow are both established and tested on a clean Windows machine.
