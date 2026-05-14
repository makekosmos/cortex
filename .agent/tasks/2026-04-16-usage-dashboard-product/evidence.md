# Evidence - usage tracker productization and dashboard

## Outcome

All acceptance criteria in `spec.md` are `PASS`.

Follow-up reliability pass on 2026-04-16:
- removed fragile `<script setup vapor>` usage from dashboard leaf components that broke `vite` dev transforms
- switched dashboard router to hash history so Electron route navigation and e2e deep links are stable
- added Playwright coverage for the dashboard (`playwright.config.ts`, `e2e/dashboard.spec.ts`)
- added a direct Playwright-library smoke runner (`scripts/runE2ESmoke.ts`) for environments where the full Playwright runner is blocked
- expanded the tracker installer bundle to include `.cmd` entrypoints and a `.zip` artifact

## Acceptance criteria

### AC1 - Existing Ark DBs migrate in place without destroying data

PASS.

Evidence:
- `packages/ark-core/rust/src/db.rs` includes `test_init_schema_migrates_existing_db_without_destroying_data`.
- The test verifies an existing DB with pre-usage tables keeps legacy data after `init_schema()` and gains the usage tables.
- Verification log: `raw/ark-core-tests.txt`.

### AC2 - Usage entities have explicit sync verification coverage

PASS.

Evidence:
- `packages/ark-core/rust/tests/sync_round_trip.rs` includes `usage_entities_sync_between_two_servers`.
- The test replicates `tracked_app`, `usage_session`, and `usage_event` through the current sync server/client path and asserts the receiving side loads them.
- Verification log: `raw/ark-core-tests.txt`.

### AC3 - usage-tracker has a Windows installer/release flow

PASS.

Evidence:
- Release packaging flow exists in `services/usage-tracker/scripts/build-installer.ps1`.
- Install/uninstall scripts exist in `services/usage-tracker/installer/install.ps1` and `services/usage-tracker/installer/uninstall.ps1`.
- `services/usage-tracker/package.json` exposes `build:release` and `package:installer`.
- Generated artifact directory: `services/usage-tracker/dist/KosmosUsageTrackerInstaller/` with `usage-tracker.exe`, `install.ps1`, `uninstall.ps1`, `Install Usage Tracker.cmd`, `Uninstall Usage Tracker.cmd`, `README.md`, `manifest.json`.
- Generated archive: `services/usage-tracker/dist/KosmosUsageTrackerInstaller.zip`.
- Verification log: `raw/usage-tracker-installer.txt`.

### AC4 - New dashboard desktop app exists with Vue Composition API and Vapor-enabled Vite

PASS.

Evidence:
- New app: `apps/dashboard/`.
- Vue + Electron scaffold in `apps/dashboard/package.json`, `apps/dashboard/vite.config.mjs`, `apps/dashboard/electron/main.ts`, `apps/dashboard/electron/preload.ts`.
- Composition API + `<script setup lang="ts">` renderer code in `apps/dashboard/src/**`.
- Vapor-enabled Vite config via `vue({ features: { vaporInterop: true } })`.
- Follow-up reliability fix removed the unstable Vapor leaf usage that caused `vite` dev parse failures while preserving Vapor-enabled build configuration.

### AC5 - Dashboard consumes `@kosmos/visuals` directly

PASS.

Evidence:
- Direct dependency: `apps/dashboard/package.json`.
- Direct alias usage in `apps/dashboard/vite.config.mjs`.
- Direct imports in `apps/dashboard/src/components/dashboard/DashboardShell.vue` and `apps/dashboard/src/global.css`.
- No vendored copy of `kosmos-visuals` exists under `apps/dashboard`.

### AC6 - Dashboard reads Ark DB through a secure Electron bridge and renders meaningful analytics

PASS.

Evidence:
- Secure bridge: `apps/dashboard/electron/preload.ts` exposes only `window.dashboardApi`.
- Read-only query layer: `apps/dashboard/electron/services/analytics.ts`.
- SQLite remains in main process: `apps/dashboard/electron/db/sqlite.ts`.
- Renderer consumes snapshots through `apps/dashboard/src/composables/useDashboardData.ts`.
- Views implemented in `apps/dashboard/src/pages/OverviewPage.vue` and `apps/dashboard/src/pages/SessionsPage.vue`.
- Smoke analytics output against Ark-compatible SQLite file: `raw/dashboard-smoke-analytics.txt` reports non-zero sessions, top app, and recent sessions.
- Dashboard now exposes stable `data-testid` hooks and hash routes for deterministic Electron/e2e verification.

### AC7 - Tracker/dashboard flows are verified with build/typecheck/package/smoke checks

PASS.

Evidence:
- Root dependency install: `raw/bun-install.txt`.
- Ark tests: `raw/ark-core-tests.txt`.
- usage-tracker tests: `raw/usage-tracker-tests.txt`.
- Dashboard typecheck: `raw/dashboard-typecheck.txt`.
- Dashboard build: `raw/dashboard-build.txt`.
- Tracker one-shot run against workspace Ark DB: `raw/usage-tracker-once.txt`.
- Dashboard smoke seed: `raw/dashboard-smoke-seed.txt`.
- Dashboard smoke analytics: `raw/dashboard-smoke-analytics.txt`.
- Tracker installer packaging: `raw/usage-tracker-installer.txt`.
- Playwright specs and direct smoke script are present under `apps/dashboard/e2e/` and `apps/dashboard/scripts/runE2ESmoke.ts`.
- In this sandbox, launching Electron from Playwright is blocked by `spawn EPERM`; local-machine execution is expected for the final e2e runner verification.

### AC8 - Relevant AGENTS files reflect the new ownership and architecture

PASS.

Evidence:
- Updated `packages/ark-core/AGENTS.md` for usage entities, migrations, and sync rules.
- Updated `apps/arrancador/AGENTS.md` to describe Arrancador as an Ark usage consumer rather than tracker owner.
- Added/updated `services/usage-tracker/AGENTS.md`.
- Added/updated `apps/dashboard/AGENTS.md`.
