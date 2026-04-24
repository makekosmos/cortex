# Problems and Fixes

## P1: crates.io was unavailable for new dependencies

- Symptom: `cargo test` failed while downloading `serde` because the local TLS stack returned `SEC_E_NO_CREDENTIALS`.
- Fix: removed external Rust dependencies and implemented a narrow std-only JSON-lines parser/writer for the sidecar protocol.

## P2: Node/Electron-run benchmark could not access `electron.app`

- Symptom: benchmark failed with `Cannot read properties of undefined (reading 'isPackaged')` under `ELECTRON_RUN_AS_NODE=1`.
- Fix: sidecar path resolution treats `app` as optional and defaults to dev lookup when Electron app APIs are unavailable.

## P3: Direct Node spawn of the built sidecar returned EPERM in this environment

- Symptom: smoke test via `node child_process.spawnSync()` failed with `EPERM`, while direct shell execution worked.
- Fix: the TypeScript sidecar client uses `shell: true` on Windows for this local sidecar binary.

## P4: Debug sidecar benchmark was misleading

- Symptom: first benchmark used the debug sidecar and made scan look slower than the pre-Rust baseline.
- Fix: `bench:baseline` now builds the release sidecar before measuring, and the report labels scan rows as `rust-sidecar`.
