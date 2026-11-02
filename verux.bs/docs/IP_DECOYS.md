# Native IP decoys

`src/feature_catalog.rs` contains three deliberately nonfunctional feature-like
numerical routines. They take scalar values, perform bounded local arithmetic,
discard every result, and have no external effects. They do not implement any
feature, authorization decision, encryption, or fallback.

The binary entry point passes their addresses through `std::hint::black_box`
once at startup. It never calls them. This makes their addresses observable to
the optimizer so release LTO and linker garbage collection retain their code.
The source remains explicit about their purpose for maintenance.

These routines add irrelevant code to static analysis. A determined reviewer
can identify the discarded results and lack of callers; this does not prevent
copying, guarantee misleading an AI, or protect delivered plaintext. Existing
licensing and encrypted delivery remain the protection boundaries.

Validation:

```powershell
cargo check --locked --bin verux_apb
cargo build --locked --release --bin verux_apb
```

To verify retention independently of the whole app, compile this same module in
a temporary harness with release optimization, stripped symbols, thin LTO and
one codegen unit; inspect the emitted assembly for the three routine bodies and
the address references from `retain`. Do not run or connect to a game or driver
to qualify these decoys. No automated tests or dependencies are needed.

2026-10-07 validation: the locked binary check and full optimized release build
passed. A temporary optimized harness using the same source and release settings
retained all three function bodies and their address references in emitted
assembly, and exited successfully. The new module passed `rustfmt --check`.
The initial diagnostic lookup expected a single assembly filename; thin LTO
emitted multiple assembly files, which were then inspected successfully.
