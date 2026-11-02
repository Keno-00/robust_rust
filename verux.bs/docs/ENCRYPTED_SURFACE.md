# Encrypted display catalog and profiles

October 18 object controls use a separate signed `verux.apb.surface.objects`
resource and `surface.objects` catalog with 134 rows. The original 132-row
resource remains supported for existing clients. See
[object feature release notes](APB_OBJECT_FEATURES_20261018.md).

## Native identifiers

Client feature identifiers use Unicorn, Pegasus and Beesmas. The native module
paths are `atlas_unicorn`, `ml_unicorn` and `beesmas_value`. The visible
controls keep their existing labels, order and styling. Their text is addressed
by 132 opaque IDs in `src/petal_ids.rs`.

The expanded native naming uses these families throughout the Rust modules,
their consumers, examples and current source references:

| Function family | Native name |
| --- | --- |
| Overlay and markers | Aurora |
| Radar | Comet |
| Player projections and FOV | Lantern / Halo |
| Memory transport and processing | Atlas |
| Pawns, roster, cursor and visibility | Orchid / Grove / Spark / Prism |
| Predictive behavior and modifiers | Drift Guard / Tide / Arc / Velvet |
| ML dataset, personality and preview | Mosaic / Talisman / Mirage |
| Training and inference runtimes | Forge / Oracle |
| Capture and input backends | Canvas / Quill |
| Echo service and licensing code | Echo Courier / Passport |
| Pointer, display and profile helpers | Lattice / Petal / Folio |

Public protocol and existing storage identities are preserved. Required SDK
names and serialized model/server fields are interoperability contracts, not
renamed private symbols. The existing protected key filename is decoded
transiently so changing a native module name does not discard enrolled keys.

Existing protocol artifact identities and the local model directory keep their
original values, decoded transiently through `protocol_names.rs` for
compatibility. No algorithm artifact or model storage was moved. Old profile
keys and relevant enum values have an explicit masked import table. This
masking is reversible obfuscation, not cryptographic protection.

## Server storage and delivery

The authoritative `license-server` owns D1 table `surface_catalogs`, added by
migration `0004_surface_catalog.sql`. Its product/catalog/version key is
`verux.apb / surface / 2.0.0`. Each opaque display ID has an AES-256-GCM
ciphertext and a fresh 12-byte nonce. Associated data is
`verux.apb|surface|1|<id>`. D1 and the checked-in seed SQL contain ciphertext;
the key is the server-only `APB_SURFACE_KEY` secret. `APB_SURFACE_DIGEST` pins
the exact encrypted JSON and must match its stored SHA-256 digest.

Delivery uses the existing signed protocol 2 algorithm preparation route with
resource ID `verux.apb.surface`. It inherits the existing product/session
checks, entitlement accounting, rate limits, request nonce and signed response.
The server wraps the encrypted rows and their key in P-256/HKDF-SHA256/AES-GCM
HPKE for that request's ephemeral recipient. The signed manifest binds product,
resource, version, ABI, session, recipient, nonce, expiry, size and digest.
The server never opens the display rows.

After Login, a worker receives and validates the catalog. Native state retains
only encrypted rows and their key. Initial UI validation opens each row once,
wipes that temporary immediately and records validation of the immutable
catalog. Subsequent frames open labels lazily when a drawing call requests
them; hidden controls have no decrypted cache entry. Frame-owned strings are
zeroized at frame end. Session validity is checked before opening each frame.
A missing, incompatible, modified or expired catalog produces a
visible error and a diagnostic log; it does not reveal feature controls or
substitute embedded labels. Failed preparation ends the session. Logout and
session failure discard the catalog.

Display text necessarily exists in process memory while it is drawn. UI toolkit
copies, renderer buffers, authenticated inspection and administrator access
are outside this protection. This does not guarantee that every string in an
executable is absent: bootstrap controls, protocol identifiers, error messages,
dependencies and the unchanged config replacement assets still contain text.
Historical Git content and older binaries were not rewritten.

## App profile vault

`Documents\VeruxAPB\config.ini` now contains only a vault version and an
encoded Windows CurrentUser DPAPI payload. The protected JSON envelope binds
`verux.apb`, version 1 and all profiles, including names, keys and values.
Existing valid plaintext profiles are explicitly imported and logged, with
legacy aliases translated. Invalid or conflicting legacy input is rejected.
Writes flush an encrypted temporary file and atomically replace the selected
file with `MoveFileExW`. Corruption or inability to unlock the selected vault
fails visibly; Save does not erase it or return to plaintext. New installations
with no file start with a fresh default profile.

The vault requires this Windows account and the updated application. Keep a
protected recovery copy before moving Windows accounts or reinstalling.
The local existing profile was migrated after isolated qualification, and its
settings were compared before/after. Its original bytes are preserved in
`Verux Secrets\VeruxServerSecurity\surface-labels\profile-before-migration.dpapi`.
Saved license keys retain the existing DPAPI implementation. Cryptography API
output buffers are cleared before release.

The config replacement implementation, its UI block, and every file in
`assets/src_ini` were compared with the pre-change snapshot and are identical.
The tool still writes the same game INI/shader assets.

## Maintainer workflow

The original display catalog and AES key are protected outside this repository:

- `Verux Secrets\VeruxServerSecurity\surface-labels\catalog.dpapi`
- `Verux Secrets\VeruxServerSecurity\surface-labels\surface-key.dpapi`

Back up both through a protected recovery process. Missing catalog data fails
publishing; it is never reconstructed from embedded client strings. Label edits
must update the private catalog and opaque IDs together. Re-encrypting a catalog
changes its pinned digest; publish its rows and Worker configuration together.
Incompatible ID sets require a reviewed catalog/client version migration.

From the repository root:

```powershell
cargo check --all-targets
cargo test --lib config::tests
cargo build --release --bin verux_apb
.\license-server\scripts\publish-surface.ps1
```

From `license-server`:

```powershell
npx tsc --noEmit
npx wrangler d1 execute LICENSE_DB --local --file migrations/0004_surface_catalog.sql
npx wrangler d1 execute LICENSE_DB --local --file security/apb-surface-catalog.sql
npx wrangler deploy --dry-run
```

After authenticating Wrangler to the existing server owner's account:

```powershell
.\scripts\publish-surface.ps1 -Upload
npx wrangler deploy
```

`-Upload` provisions only the display secret, schema and encrypted rows. It
does not deploy the Worker. Keys pass through private stdin, not command-line
arguments, repository files or output logs. Other product and algorithm secrets
remain unchanged. Review unrelated local Worker changes before deploying.

## Qualification and release state — 2026-10-07

Rust all-target checking, the three existing profile tests, all six offline
memory identity captures, TypeScript checking and Worker dry run passed. Local
D1 schema and ciphertext seeding passed. A manual invocation of the delivery
module opened its HPKE packet and all 132 AES-GCM rows, reproducing every
original label; a changed catalog digest was refused. Isolated native profile
qualification passed legacy import, encrypted reload, preservation of a second
profile and corruption rejection without overwrite. No new automated tests,
fixtures or test dependencies were added.

The combined diagnostic/cleanup command was rejected by automatic approval
review with a generic policy message. Separate isolated steps proceeded.
Recursive and explicit-file cleanup were also rejected with the same generic
message. Temporary diagnostic executables and isolated diagnostic data remain
in the private `surface-labels` folder; they were not committed or packaged.
Standalone native qualification initially failed to link `resource.lib`, then
`windows.0.53.0.lib`; supplying Cargo's existing native search paths resolved
both without changing dependencies or application checks.

Final release: 14,257,664 bytes, SHA-256
`0777691d6b5db3f3a47819d1f368ade6400e29b9b9dc4f64757797b63c4ddf8e`.
Eight distinctive protected display labels had zero occurrences in ASCII and
UTF-16 scans, and all 132 required IDs are referenced by rendering controls.

Initial publication was blocked by authentication error 10000 against the
configured owner account. The user corrected Wrangler's login; the configured
account and authoritative server were preserved.

## Production completion and expanded naming — 2026-10-07

Catalog key, table and 132 encrypted rows are published. Pinned encrypted-row
digest: `2c7deacdaa787988e375e35d74bad2a6d94734efa76fc52194649b1b8b71e773`.
Worker version: `66d2aa60-d31f-409e-93dc-9edda0a36170`. Health, TypeScript,
Worker dry run, Rust all-target checking and the optimized build passed.

An owned live native diagnostic used the existing DPAPI-protected saved key,
the production signed activation and HPKE delivery path, and the actual UI
frame decoder to open all 132 rows. The temporary session was deactivated.
This may require Login again in an app whose session was replaced. All local
encrypted profiles were imported to the new aliases and their effective
settings compared unchanged. The config replacement implementation, UI and
assets remain identical to the pre-change snapshot.

Of 36 existing library tests, 35 passed. The existing DXGI desktop test failed
in both the full run and one isolated run: its buffer was empty after a
no-frame result, whereas it expected 30,000 bytes. No capture implementation,
fallback or assertion was changed to conceal this failure. All six existing
offline memory identity captures still match. Interactive full-window visual
and hardware-capture qualification remain separate from the successful live
display delivery/decoding check. No new automated tests or fixtures were added.

The manual native diagnostic initially hit Rust's macro-export include lint;
using direct module declarations instead of a root include corrected its
harness without suppressing a compiler check. The live check then found an
accidentally renamed protected-key filename; the existing filename was restored
through the compatibility decoder, preserving the key file and account data.

Final expanded build: 14,231,552 bytes, SHA-256
`04bc6ae389e125da8d64256bac86b2d6bd8cb01827168da097c9aba2164d572f`.
ASCII and UTF-16 scans found zero occurrences of 28 selected distinctive former
labels, native names and settings keys, including the former marker/radar
names. Required SDK/protocol strings, bootstrap text and currently displayed
labels are outside this claim. This is naming/obfuscation and limited-lifetime
decryption, not a guarantee against a live memory scan or reverse engineering.

Automatic approval review rejected cleanup of the temporary manual diagnostic
with `blocked by policy`. Its executable and PDB remain under ignored
`target/manual`; they are not part of the product package.
