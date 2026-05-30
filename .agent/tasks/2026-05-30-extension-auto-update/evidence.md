# Evidence — 2026-05-30 Extension Auto Update

Verified at: 2026-05-30

## AC1 — PASS

Production startup path in `shell/electron/main.ts` already calls `startPeriodicCatalogCheck()` only when `KOSMOS_TEST_MODE !== "1"` and `KEPLER_INSTANCE.periodicMarketplaceCheckEnabled` is true. `startPeriodicCatalogCheck()` now calls `autoUpdateExtensionsOnce(false)` immediately, so startup performs unattended update without Settings UI or user confirmation.

Commands:

- `bun run --cwd shell build:js` — PASS

## AC2 — PASS

`autoUpdateExtensionsOnce()` calls `installFromUrl(candidate.downloadUrl, candidate.sha256)`, and `installFromUrl()` downloads/validates SHA-256 before delegating to `installFromPath()`. Existing backup + atomic install logic remains in `extension-installer.ts`; no extraction/backup copy was added elsewhere.

Commands:

- `bun run --cwd shell typecheck` — PASS
- `bun run --cwd shell build:js` — PASS

## AC3 — PASS

`findExtensionUpdates()` filters to `source === "installed"`, requires catalog entry, requires valid `MAJOR.MINOR.PATCH` semver for both sides, and only returns candidates where catalog version is strictly greater than installed version.

Commands:

- `bun test tests/unit/extension-update-plan.test.ts` — PASS, 2 tests passed.

## AC4 — PASS

After successful background installs, `autoUpdateExtensionsOnce()` calls the shared local `notifyCommandsChanged()` broadcaster, same channel as manual marketplace install: `kepler:commands:updated`. It also calls `reloadExtensionWindow(id)` so an already-open Vue extension reloads onto the installed bundle without user navigation.

Commands:

- `bun run --cwd shell typecheck` — PASS
- `bun run --cwd shell build:js` — PASS

## AC5 — PASS

`autoUpdateExtensionsOnce()` wraps each candidate install in its own `try/catch`, logs `[marketplace] extension auto-update failed for <id>`, and continues the loop. Catalog fetch failure is caught and returns without throwing into startup.

Commands:

- `bun run --cwd shell typecheck` — PASS

## AC6 — PASS

Commands:

- `bun test tests/unit/extension-update-plan.test.ts` — PASS, 2 tests passed.
- `bun run --cwd shell typecheck` — PASS.
- `bun run --cwd shell build:js` — PASS.
- `bun run docs:check` — PASS.
- `bun run format:check shell/electron/extension-marketplace.ts shell/electron/extension-update-plan.ts shell/electron/extension-host.ts tests/unit/extension-update-plan.test.ts docs-site/concepts/distribution.md docs-site/concepts/extension-installer.md STATUS.md .agent/tasks/2026-05-30-extension-auto-update/spec.md .agent/tasks/2026-05-30-extension-auto-update/evidence.md .agent/tasks/2026-05-30-extension-auto-update/evidence.json` — PASS.

Build note: existing warnings remain unrelated: Lightning CSS warning for `.settings-shell .advanced-page__body :deep(...)` and Vite/Rolldown `inlineDynamicImports` warning.
