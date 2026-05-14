# Task Spec - usage tracker productization and dashboard

## Original task statement
- Ensure usage sync is safe and does not break when databases merge across devices.
- Update `AGENTS.md` files in directories touched by the work so instructions match the current code.
- Create an installer for the activity tracker.
- Add a separate `apps/dashboard` application using the latest Vue stack with Vapor mode.
- Use `@kosmos/visuals` directly instead of copying shared UI objects into the app.
- Build a dashboard product that clearly visualizes what usage/time data currently exists in Ark DB.
- Extend one of the already existing databases instead of introducing a parallel usage database.

## Goal
Turn the extracted usage tracking work into a usable product surface: harden Ark-side migration and sync coverage for usage entities, package the Windows tracker for installation, and ship a separate Electron + Vue/Vapor dashboard app that reads the existing Ark DB and visualizes usage analytics through a secure read-only adapter.

## Assumptions
- The existing Ark SQLite database remains the single source of truth for usage data.
- `apps/dashboard` should be a desktop application, not a browser-only SPA, because it needs local read access to `ark.db`.
- Electron + Vue 3 with Vapor mode is the pragmatic implementation path for `apps/dashboard` in this repo.
- `@kosmos/visuals` should be consumed by workspace alias/dependency, not copied into the app.
- If MSI/WiX tooling is unavailable in the environment, a production-oriented Windows install bundle with release binary plus install/uninstall scripts is an acceptable installer outcome for this pass.
- Sync safety for this pass means: schema migration on existing Ark DBs is idempotent, usage entities are covered by Ark sync tests, and there is no separate non-syncing usage storage path.

## Acceptance Criteria
- AC1: `packages/ark-core/rust` can initialize an existing Ark DB and add usage tracking schema without destroying existing data.
- AC2: Usage entities (`tracked_app`, `usage_session`, `usage_event`) have explicit sync verification coverage beyond local CRUD, so their replication path is tested against the current Ark sync code.
- AC3: `services/usage-tracker` has a documented Windows release/installer flow that produces a distributable install bundle from the repo.
- AC4: A new `apps/dashboard` desktop app exists and uses Vue Composition API with Vapor-mode-enabled Vite config.
- AC5: `apps/dashboard` consumes `@kosmos/visuals` directly and does not vendor/copy shared visuals into the app.
- AC6: `apps/dashboard` can read the existing Ark DB through a secure Electron bridge and render meaningful usage analytics views (summary cards, trend charts, top apps, recent sessions, or equivalent high-signal slices).
- AC7: The dashboard and tracker flows are verified with targeted build/typecheck/package checks, and at least one smoke path exercises the analytics against a real Ark-compatible SQLite file.
- AC8: Relevant `AGENTS.md` files are updated to reflect the new usage-tracker ownership, dashboard app, and Ark usage-sync/storage behavior.

## Constraints
- Keep all workflow artifacts under `.agent/tasks/2026-04-16-usage-dashboard-product/`.
- Do not introduce a second source of truth for usage data outside Ark DB.
- Preserve secure Electron boundaries: renderer must not get raw unrestricted IPC access.
- Prefer the smallest defensible schema expansion over speculative analytics tables unless needed for product behavior.
- Use Vue Composition API with `<script setup lang="ts">` in the new dashboard app.

## Non-goals
- Full cross-platform tracker backend support beyond Windows.
- A cloud-hosted analytics service.
- Replacing existing Delphi or Arrancador product surfaces.
- Solving pre-existing unrelated test failures in other apps unless they block this task directly.

## Bounded implementation plan
1. Add migration/sync-hardening coverage in Ark for usage entities and existing DB initialization.
2. Add a Windows installer/distribution workflow for `services/usage-tracker`.
3. Scaffold `apps/dashboard` as Electron + Vue/Vapor with a secure preload/main-process API.
4. Implement read-only Ark usage queries and dashboard analytics UI using `@kosmos/visuals`.
5. Update relevant `AGENTS.md` files and task evidence.
6. Run targeted verification for Ark, tracker packaging, and dashboard build/smoke flows.

## Verification plan
- Run Ark Rust tests covering schema migration and usage sync.
- Build/test the tracker release/installer bundle at least to artifact generation.
- Typecheck/build/package the dashboard app.
- Exercise dashboard analytics against a real Ark-compatible SQLite file created or migrated by current code.
- Re-read updated `AGENTS.md` files and ensure they reflect only behavior that exists in code now.
