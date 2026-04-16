# AGENTS.md

Project: Dashboard (Electron + Vue)

Purpose
- Desktop observability app for Ark DB usage data: foreground sessions, recorded time, app ranking, and recent session playback.

Tech stack
- Frontend: Vue 3 + Composition API + `<script setup lang="ts">`
- Runtime: Electron
- Data access: read-only SQLite via `better-sqlite3` in Electron main process
- Shared UI: `@kepler/visuals`

Repo layout
- `src/App.vue` Root composition surface only
- `src/components/dashboard/` Product UI sections and charts
- `src/components/dashboard/DashboardShell.vue` Product shell; consumes shared `DesktopChrome`, `Titlebar`, `DesktopContentSurface`, and `StatusDot` from `@kepler/visuals`
- `src/pages/` Route-level composition pages
- `src/composables/useDashboardData.ts` Shared dashboard state + renderer actions
- `src/utils/format.ts` Pure formatting helpers
- `shared/analytics.ts` Shared IPC contracts and analytics payload types
- `electron/main.ts` BrowserWindow + narrow IPC handlers
- `electron/preload.ts` Safe bridge exposed as `window.dashboardApi`
- `electron/services/analytics.ts` Read-only Ark DB queries and snapshot assembly
- `electron/db/sqlite.ts` Node-side SQLite wrapper
- `scripts/seedSmokeDb.ts` Smoke fixture seeding for analytics verification
- `scripts/seedSmokeDb.py` Playwright-safe smoke fixture seeding without native Node SQLite ABI dependency
- `scripts/smokeAnalytics.ts` CLI smoke assertion for dashboard analytics

Rules
- Renderer MUST NOT open SQLite directly; all DB reads go through `window.dashboardApi`.
- `electron/services/analytics.ts` is the single source of truth for dashboard SQL; do not duplicate analytics queries inside Vue components.
- Use `@kepler/visuals` by import/alias only; never copy shared sidebar or tokens into `apps/dashboard`.
- Desktop window chrome should stay aligned with shared visuals components; wire actions in dashboard, but keep titlebar/sidebar layout primitives in `@kepler/visuals`.
- Keep route components thin: they compose sections, but data fetching stays in `useDashboardData`.
- Prefer presentational leaf components for charts/cards; keep side effects in composables or Electron main.
- Keep router navigation hash-based so deep-linking and Electron e2e navigation remain stable.

Packaging
- `bun run build` builds renderer + Electron bundles.
- `bun run package:dir` creates an unpacked desktop bundle.
- `bun run dist` targets Windows NSIS through electron-builder.
- `bun run smoke:seed` and `bun run smoke:analytics` are verification helpers and run through Node type-stripping, not Bun, because `better-sqlite3` is used in the smoke path.
- `bun run test:e2e` is the normal Playwright path on a local machine; Playwright `globalSetup` seeds the smoke DB through `scripts/seedSmokeDb.py` to avoid `better-sqlite3` ABI drift in Node-side setup.
- `bun run test:e2e:smoke` is a direct Playwright-library smoke script useful when the full Playwright runner is blocked by environment process restrictions.
