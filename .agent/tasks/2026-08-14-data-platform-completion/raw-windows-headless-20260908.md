# Windows packaged headless evidence — 2026-09-08

All runs used isolated temporary data and `KOSMOS_HEADLESS=1`; no registry,
user data, visible GUI, physical microphone, or Groq path was exercised.

## Delivered inputs

- Cortex bridge/runtime source: `0956455e80bc2aa5359c4254bfe2971fc361451e`
- Engine release: `v0.1.1`; archive SHA-256
  `5144c434e8f85dfe55c011843d1eaf7c89a89e19bfb45352fca507e981de873f`
- Package Index: catalog18; catalog SHA-256
  `1e65f56a3d5f6cb382619d4f8211a4dc5a6fdac019eb1c5592b99072a68c633f`
- Store: catalog17 release, catalog asset SHA-256
  `f49ee76f44d3f3aef12829ba8e1841652e6ce1f6f2347c467782f79498922550`;
  Dictation `0.2.5`, Memoria `0.6.6`.

## Packaged smoke

Command: `node scripts/candidate-installed-smoke.mjs release/win-unpacked`

Result: `pass`; public catalog `default`, Package Index sequence `18`;
Manager, Host, and Runtime owned processes started with no visible windows;
the install/update/uninstall path completed and `afterCleanup` reported empty
process/window/package state. Store refresh was accepted after catalog17
aligned the Store listings with Package Index catalog18.

## Supporting targeted suites

- `bun test` package-release, migration journal/namespace, Store helpers,
  Manager surface and Manager IPC contracts: `53 passed, 0 failed`.
- `cargo test --package kepler-backend --test package_worker_windows
  --features package-worker-fixture`: `12/12`.
- `cargo test --package kepler-backend --test package_worker_process_windows
  --features package-worker-fixture`: `20/20`.
- `cargo test --package kepler-backend --test cortex_2_acceptance
  --features package-worker-fixture`: `7/7`.
- `cargo test --package kepler-backend --test desktop_authority_socket
  --features package-worker-fixture`: `6/6`.
- `cargo test --package kepler-backend --test phase5_runtime_grants`: `6/6`.
- `cargo test --test store_catalog_runtime`: `12/12`.
- `cargo test --test ark_markdown_bridge_worker`: `1/1`.
- `cargo test --manifest-path packages/ark-markdown-bridge/Cargo.toml`:
  worker/unit/contract suites `11/11`.

