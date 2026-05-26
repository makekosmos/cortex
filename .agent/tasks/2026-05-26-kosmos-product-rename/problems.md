# Problems

## P1 — ARK guard false positive on legacy install path

Initial `bun run ark:guard:writes` failed:

```text
=== path.join(..., 'Kosmos'|'Kepler', ...) outside instance.ts / data-dir.ts ===
shell/electron/settings-window.ts:184: paths.push(path.join(localAppData, "Programs", "Kepler", "Kepler.exe"));
```

Root cause: the guard intentionally rejects hardcoded `path.join(..., "Kepler")`
outside `instance.ts` because userData paths must go through instance
resolution. In this case the path was not userData or ARK data; it was a legacy
install executable candidate for autostart migration.

Fix: changed the legacy install candidate construction to `path.resolve(...)`,
keeping behavior but avoiding the write-boundary/userData guard pattern.

Reverify:

```powershell
bun run ark:guard:writes
bun run --cwd shell typecheck
bun run ark:smoke
```

Result: PASS.

## P2 — uninstall удалял только один service name

Review found that `services/kepler-focus-svc/src/cli.rs::uninstall()` reused
the compatibility helper that returns the first installed service. That is
correct for `status/start/stop`, but wrong for cleanup: after upgrade both
`KosmosSystemSvc` and legacy `KeplerFocusSvc` may exist.

Impact: uninstall could delete `KosmosSystemSvc` and leave `KeplerFocusSvc` in
SCM, so Settings would still show the service as installed.

Fix: `uninstall()` now iterates `[KosmosSystemSvc, KeplerFocusSvc]` and
best-effort stops/deletes each installed service. Added regression test
`uninstall_targets_new_and_legacy_service_names` and postmortem entry
`docs-site/agents/postmortems.md § 2026-05-26`.

Reverify:

```powershell
cargo test -p kepler-focus-svc uninstall_targets_new_and_legacy_service_names
cargo build -p kepler-focus-svc -p kepler-focus-helper -p kepler-backend
```

Result: PASS.

## P3 — kext e2e требовали ручную чистку старых data dirs

Full `bun run --cwd shell test:e2e` initially failed before three kext
scenarios started because fixed `.e2e/kepler-data-*` folders contained stale
`kepler.lock.json` files that the current user could not delete (`EPERM`).

Fix: kext e2e specs now create per-run `KOSMOS_DATA_DIR` and userData folders
using `process.pid` + timestamp. Tests no longer depend on deleting old
artifacts.

Reverify:

```powershell
bunx playwright test --config shell/playwright.config.ts shell/e2e/kext-argv.spec.ts shell/e2e/kext-install.spec.ts shell/e2e/kext-revert.spec.ts
bun run --cwd shell test:e2e
```

Result: PASS, 12/12 e2e.

## P4 — release script could package stale helper/service binaries

Review found that `shell/package.json::build:backend` rebuilt only
`kepler-backend` and `ark-core-rpc`, while `extraResources` also packages
`kepler-focus-helper.exe` and `kepler-focus-svc.exe`. After service/helper code
changes, `bun run --cwd shell build` could therefore package stale binaries
from a previous `target/release`.

Fix: `build:backend` now builds all four packaged Rust executables:
`kepler-backend`, `ark-core-rpc`, `kepler-focus-helper`, `kepler-focus-svc`.

Reverify:

```powershell
cargo build --release --manifest-path Cargo.toml --bin kepler-backend --bin ark-core-rpc --bin kepler-focus-helper --bin kepler-focus-svc
```

Result: PASS after stopping legacy `KeplerFocusSvc`.
