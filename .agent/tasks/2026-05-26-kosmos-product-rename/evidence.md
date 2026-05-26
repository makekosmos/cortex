# Evidence — Kosmos Product Rename Migration

Verified at: 2026-05-26T22:36:00+03:00.

## AC1. Product naming — PASS

Evidence:

- `shell/package.json` production config now uses `productName: "Kosmos"`,
  `appId: "com.kazui.kosmos"`, NSIS `shortcutName: "Kosmos"`, and renamed
  packaged resources:
  - `Kosmos Runtime.exe`
  - `Kosmos Data Engine.exe`
  - `Kosmos Helper.exe`
  - `Kosmos System Service.exe`
- `shell/package.json` `build:backend` now builds all Rust executables that
  are packaged as resources, including focus helper/service.
- `shell/electron/main.ts::resolveBackendExe()` resolves packaged
  `Kosmos Runtime.exe` first, then legacy `kepler-backend.exe`.
- `services/kepler-backend/src/ark_host.rs` resolves packaged
  `Kosmos Data Engine.exe` first, then legacy `ark-core-rpc.exe`.
- `shell/index.html` and Settings window title use `Kosmos`.

Verification commands:

```powershell
bun run --cwd shell typecheck
```

Result: PASS.

## AC2. Existing user migration — PASS

Evidence:

- `shell/electron/instance.ts` prod `userDataDir` is now
  `%APPDATA%/Kosmos App`.
- `migrateLegacyProdUserData()` copies legacy `%APPDATA%/Kepler` Electron
  userData into `%APPDATA%/Kosmos App` only when the new directory is missing
  or empty.
- ARK `dataDir` remains `%APPDATA%/Kosmos`.

Verification command:

```powershell
bun run --cwd shell typecheck
```

Result: PASS.

## AC3. Autostart migration — PASS

Evidence:

- `shell/electron/settings-window.ts` writes new login item with
  `name: "Kosmos"`.
- `isAutostartEnabled()` treats detected legacy `Kepler.exe --autostart` as
  enabled.
- `setAutostartEnabled()` best-effort removes legacy Kepler login entries.

Verification command:

```powershell
bun run --cwd shell typecheck
bun run ark:guard:writes
```

Result: PASS after fixing one guard false positive (see `problems.md`).

## AC4. Service naming compatibility — PASS

Evidence:

- `services/kepler-focus-svc/src/cli.rs` new service name/display name:
  `KosmosSystemSvc` / `Kosmos System Service`.
- CLI `status/start/stop/uninstall` resolves either new `KosmosSystemSvc` or
  legacy `KeplerFocusSvc`.
- CLI `uninstall` removes both new and legacy service names best-effort, so an
  upgraded machine cannot keep a stale `KeplerFocusSvc` after uninstall.
- Service process can start under either service name.
- Pipe moved to `\\.\pipe\kosmos-system-service`; shell/backend keep fallback
  to legacy `\\.\pipe\kepler-focus-svc`.

Verification command:

```powershell
cargo build -p kepler-focus-svc -p kepler-focus-helper -p kepler-backend
cargo test -p kepler-focus-svc uninstall_targets_new_and_legacy_service_names
```

Result: PASS. Build emitted an existing non-fatal `dead_code` warning in
`kepler-backend::dictation`.

## AC5. Verification — PASS

Commands:

```powershell
bun run --cwd shell typecheck
cargo build -p kepler-focus-svc -p kepler-focus-helper -p kepler-backend
bun test packages/ark/tests/ensure-kepler.test.ts
bun run docs:check
bun run ark:guard:writes
bun run ark:smoke
cargo test -p kepler-focus-svc uninstall_targets_new_and_legacy_service_names
cargo build --release --manifest-path Cargo.toml --bin kepler-backend --bin ark-core-rpc --bin kepler-focus-helper --bin kepler-focus-svc
bun run --cwd shell package:dir
bun run --cwd shell test:e2e
```

Results:

- `bun run --cwd shell typecheck` — PASS.
- `cargo build -p kepler-focus-svc -p kepler-focus-helper -p kepler-backend`
  — PASS, one existing non-fatal dictation dead_code warning.
- `bun test packages/ark/tests/ensure-kepler.test.ts` — PASS, 12/12.
- `bun run docs:check` — PASS.
- `bun run ark:guard:writes` — PASS.
- `bun run ark:smoke` — PASS. Smoke emitted existing warnings from
  lightningcss/plugin timings/deprecation but completed successfully.
- `cargo test -p kepler-focus-svc uninstall_targets_new_and_legacy_service_names`
  — PASS.
- Standard release Rust build in `target\release` — PASS after stopping legacy
  `KeplerFocusSvc`.
- `bun run --cwd shell package:dir` — PASS, produced
  `shell/release/win-unpacked/Kosmos.exe`.
- `bun run --cwd shell test:e2e` — PASS, 12/12. Kext e2e tests now use fresh
  per-run data dirs so stale inaccessible lock files do not require manual
  cleanup.

## AC6. Docs — PASS

Evidence:

- Updated `STATUS.md`, `docs-site/concepts/architecture.md`,
  `docs-site/apps/kepler.md`, `docs-site/concepts/instances.md`,
  `docs-site/concepts/system-requirements.md`,
  `docs-site/concepts/focus-mode.md`, `docs-site/agents/forbidden.md`,
  `docs-site/agents/claude-md-core.md`, `docs-site/reference/rules.md`,
  `docs-site/reference/commands.md`, `docs-site/services/index.md`.
- Ran `bun run docs:sync`, which regenerated `AGENTS.md`, `CLAUDE.md`,
  `crates/ark-core/AGENTS.md`, `mobile/delphi/AGENTS.md`, and llms files.
- Docs explicitly state that merging `kepler-backend` and `ark-core-rpc` into
  one process is not part of this migration.

Verification command:

```powershell
bun run docs:check
```

Result: PASS.
