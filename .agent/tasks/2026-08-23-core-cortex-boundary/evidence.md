# Evidence

## AC1 — PASS

`cortex/Cargo.toml` is a Rust workspace for `runtime` and all three native
services. Their `workspace = true` entries resolve in Cortex.

## AC2 — PASS

`cortex/runtime/Cargo.toml` has one explicit `ark-core` path dependency:
`../../core/core/ark/crates/ark-core/rust`. ARK sources were not copied to
Cortex.

## AC3 — PASS

```text
cd C:\Users\kirill\Coding\Makekosmos\cortex
cargo check -p kepler-backend
Finished `dev` profile [unoptimized + debuginfo]
```

## AC4 — PASS

```text
cd C:\Users\kirill\Coding\Makekosmos\core
cargo check -p ark-core
Finished `dev` profile [unoptimized + debuginfo]
```

An explicit path check also confirmed that `platform/desktop`,
`platform/runtime`, and `platform/native-services` do not exist in Core.

## AC5 — PASS

The Core and Cortex READMEs document the boundary and source dependency.
The stale `platform/desktop`, old repository URL, and Dictation ownership
references were removed. Independent review was repeated after that cleanup.
