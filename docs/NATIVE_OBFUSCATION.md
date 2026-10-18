# Native release obfuscation

The release profile already strips symbols, uses thin LTO and one codegen unit,
and aborts on panic. Native offsets and target addresses already have separate
build/process masking. Feature labels retain their authenticated encrypted
catalog; licensing and server-only secrets stay in the authoritative server.

Compatibility text now uses separate Windows-RNG build material instead of the fixed
one-byte mask. Every legacy profile alias and the three existing artifact/model/
license-file names are re-encoded at compile time, decoded through a non-inlined
runtime boundary, and opened in zeroizing temporary strings. Existing names,
saved profiles, product identities and network contracts remain identical.
The two public-facing name helpers still return owned strings to preserve their
existing callers; those returned copies are not guaranteed to be wiped.

This is reversible masking, not cryptographic protection. Material changes when
Cargo reruns the build script, not on every cached build. The executable contains
the decoder and sufficient material to recover these names. This pass does not
transform arbitrary function control flow or conceal DLLs.

Build with:

```powershell
.\scripts\build-obfuscated.ps1
```

The script remaps repository, Cargo and toolchain source paths for the full
Rust build, including dependencies, and applies MSVC path mapping to x64 C++
dependencies so ImGui assertion paths are also remapped without disabling
assertions. It retains release optimizations and writes a
separate executable to `target/obfuscated/release/verux_apb.exe`, so the running
standard release is not replaced. It preserves and restores encoded compiler
flags and x64 MSVC C++ flags, refuses ambiguous plain RUSTFLAGS, and reports
build failures directly.
Windows DLL/resource names, diagnostic messages and embedded assets remain.
The generated executable must go through the existing approved signing process
before distribution; this command does not sign or publish it.

Validation uses compilation, existing config tests, compatibility-table
comparison and inspection of the resulting binary. No new automated tests,
fixtures, dependencies, UI changes or fallback implementations are added.

## Qualification — 2026-10-08

```powershell
cargo check --locked --bin verux_apb
cargo test --locked --lib config::tests
.\scripts\build-obfuscated.ps1
Get-FileHash .\target\obfuscated\release\verux_apb.exe -Algorithm SHA256
```

Compilation and all three existing config tests passed. Binary inspection found
0/59 old fixed-mask alias byte sequences, 0/3 selected plaintext compatibility
names in ASCII/UTF-16, no repository/Cargo/toolchain root paths in either slash
form or encoding, and zero PE COFF symbols. The previous release contained all
59 old alias sequences and the absolute repository path. The finished executable
is 14,228,480 bytes; SHA-256:
`f38077bd6c90a66493701e2355ee38de4d0ab46fd29f44619c0ea35345ba5ef6`.

The first build failed after generated-material changes overlapped the running
build; a fresh run corrected the stale generated output. Initial native path
inspection found ImGui UTF-16 assertion paths. MSVC initially ignored quoted
arguments, then reported that path mapping requires deterministic compilation.
Enabling cc-rs shell argument parsing and MSVC deterministic compilation resolved
both; the native library and final executable were checked again. Obsolete
compiler runs were explicitly stopped during these corrections. No assertions
were disabled and no successful result was claimed for the failed inspections.

This separate executable is unsigned. Live startup/login and game/driver use were
not exercised; the existing running release was preserved.

The native compiler mapping follows the MSVC configuration used by
[OpenJDK's build](https://github.com/openjdk/jdk/blob/master/make/autoconf/flags-cflags.m4).
