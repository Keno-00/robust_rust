# Native pointer and offset encoding

Implemented 2026-10-07 for Verux APB. This covers our client representation;
it does not decode encrypted APB pointers or change EchoDriver's read protocol.

## Implementation

`src/lattice_encoding.rs` provides two data-only representations:

- `EncodedOffset`: native camera/cursor RVAs, arithmetic field offsets,
  visibility offsets and object-type vtable RVAs are stored as XOR/rotation
  encoded constants. `build.rs` generates fresh masks with Windows
  `BCryptGenRandom` whenever Cargo executes the build script. Generated material
  is under ignored `target/.../out`, never supplied through license secrets.
- `EncodedAddress`: cached module bases, camera/table bindings, pawn identities,
  World/controller/possession ownership, roster heads/links and projected pawn
  identities use a separate key generated once per Verux process. Their private
  representation is encoded; `.get()` supplies the original integer for guarded
  reads, comparisons and explicitly requested diagnostics.

Both use `encoded = rotate_left(value XOR mask_a, rotation) XOR mask_b`.
Copied instance fingerprints are also masked because they contain pointers.
Equality/hash identity remains consistent within a process. Numerical ordering
uses decoded addresses where the old code required address ordering.

The existing Windows RNG helper supplies runtime material. Startup seed buffers
use `Zeroizing`; process key ownership is static for the lifetime of all encoded
addresses. The key must not be rotated underneath existing caches. It is not
stored in config or sent to the server. Build/RNG failure fails explicitly;
there is no zero-key, predictable-key or unencoded substitute.

Decoding functions are not inlined and use `black_box` to discourage constant
folding. That is a compiler hint, not a security guarantee. Verify optimized
artifacts after compiler/toolchain changes.

## Scope and limits

This is reversible obfuscation, not authenticated encryption, code encryption,
tamper protection or a license boundary. A user controlling the client can
recover the masks, inspect decoded values at use, or modify native code. Raw
EchoDriver response buffers and short-lived read/traversal integers still
contain original data. Fixed buffer sizes, class IDs, read protocol constants,
source files and saved diagnostic captures are not made secret. Explicit
hex-address logs still expose those diagnostic values; generic `Debug` for the
encoded representations does not print their payloads.

Rust references, function pointers, OS handles and executing native machine code
are not mangled. This avoids changing their ABI or pointer provenance. Native
UI style and existing license/session/signature checks are preserved. Delivered
algorithm modules continue to use their existing signed HPKE-encrypted delivery
and bounded Wasmtime execution. This encoding does not replace any of those
controls or duplicate the license server.

## Validation

No new tests, fixtures or dependencies were added. Reproducible commands:

```powershell
cargo check --bin verux_apb --example apb_memory_probe
cargo build --release --bin verux_apb
cargo run --example apb_memory_probe -- --capture-root '..\drayebr\build-rpm\snapshots'
```

The existing probe replays the six saved character/World roster captures and
checks identities, object types, linked-list edges and captured character sets.
This is offline validation; live APB input/performance qualification remains a
separate check. Release literal inspection checks the selected camera/cursor
and type RVAs; it does not establish that all meaningful constants are hidden.

Validation on 2026-10-07:

- App and existing probe `cargo check`: passed without warnings.
- Optimized app build: passed. Eight selected RVAs (camera/cursor roots and six
  object types) had zero plain little-endian 32-bit and 64-bit occurrences in
  the inspected release executable.
- Existing offline probe: all six captures passed; character counts were
  6, 6, 7, 8, 7 and 2, with every captured subset agreeing with rooted traversal.
- Focused `git diff --check`: passed; Git reported only its existing LF/CRLF
  conversion notices.

Inspected local release SHA-256:
`a9ff39aeac0d4d5d708c7b670ae3f7269fa481591011088065d561d1d9b6de11`.
This identifies the local validation artifact, not a signed/published release.

References: [Windows RNG](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptgenrandom),
[Rust compiler hint limits](https://doc.rust-lang.org/std/hint/fn.black_box.html).
