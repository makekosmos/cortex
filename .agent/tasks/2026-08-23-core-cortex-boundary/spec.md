# Core / Cortex ownership split

## Goal

Make `core` the sole owner of ARK data code and make `cortex` independently
own the Desktop host, Manager, runtime and native services.

## In scope

- Add a Cargo workspace to `cortex` for its runtime and native services.
- Keep `ark-core` as an explicit path dependency from `cortex` to `core`.
- Remove stale `platform/desktop`, `platform/runtime`, and
  `platform/native-services` copies from `core` after Cortex's checks pass.
- Update ownership documentation and repository metadata.

## Out of scope

- Changing ARK schema, sync behaviour, IPC wire formats, or package names.
- Releasing binaries or publishing npm packages.
- Moving product repositories or changing user settings.

## Acceptance criteria

- **AC1.** `cortex/Cargo.toml` defines a workspace containing its runtime and
  all native services, so their `workspace = true` Cargo settings no longer
  resolve through `core`.
- **AC2.** `cortex/runtime` retains one explicit dependency on ARK at
  `../../core/ark/crates/ark-core/rust`; no ARK source is copied into Cortex.
- **AC3.** `cargo check -p kepler-backend` succeeds from the Cortex repository.
- **AC4.** Core no longer contains `platform/desktop`, `platform/runtime`, or
  `platform/native-services`; its Cargo workspace contains ARK only.
- **AC5.** Core and Cortex READMEs describe the final ownership boundary.
