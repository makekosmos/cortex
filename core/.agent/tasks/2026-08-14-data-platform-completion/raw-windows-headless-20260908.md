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

## Published Memoria crash/restart follow-up — 2026-09-08

The published package defect found against catalog18 was fixed in Memoria
`v0.6.7` (merged PR #12, `e04ebd8bc3dab07956289ad23a257fc76a285194`). The
release archive is immutable: SHA-256
`d1b5f39c6f06b2b6a760e27ed7ece49f3d8d51079d8901f8f1c0c080737c9023`, size
`2886184`.

Package Index PR #36 was merged as `833c70ece4ae709fc8a13788fb19a9b50cb5e700`
and the production `catalog-19` workflow passed (`34260584066`). The signed
catalog asset SHA-256 is
`fbfc5853e75ec0f2f45cdc4ef7790ebdc217cf658fb91c166bec5fd8b0baa265`.

Exact command:

`bun run --cwd host e2e first-party-memoria-import-crash.spec.ts --workers=1`

The test used the v0.9.22 source Host with real contextBridge IPC, production
catalog19/signatures, published Memoria 0.6.7, and the packaged v0.1.2 Engine
and ARK binaries from Desktop v0.9.22. `packages.trust_status`,
`catalog_apply`, `install`, and `set_enabled` all returned `ok=true`; the
import marker was reached, the Engine was crashed at the durable-step boundary,
Host and Engine restarted, and the baseline entry set was restored. Result:
`PASS`, 1/1, 21.7s, with temporary data and process cleanup complete.

The exact packaged backend inputs were Engine SHA-256
`76644e8625d19cfeea1811ac0e6cab50fb0733009e388cd245673386f9aebb9d` and ARK
SHA-256 `266b74a334337812fcabd82cb30eceee98c857a2abb9dd2a1039a99b9bb036d6`.
The harness timeout-only follow-up is Cortex commit `69bed8d75f73a17ed77af83fb3ac0034d0d77e27`
(PR #51); no product/runtime release bytes changed.
