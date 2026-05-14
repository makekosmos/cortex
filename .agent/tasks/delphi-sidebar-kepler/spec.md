# Task Spec: delphi-sidebar-kosmos

## Metadata
- Task ID: delphi-sidebar-kosmos
- Created: 2026-04-10T23:09:17+00:00
- Repo root: /workspace
- Working directory at init: /workspace

## Guidance sources
- AGENTS.md

## Original task statement
Привести apps/delphi/ts/src/components/SideBar.vue к визуалу sidebar.vue из packages/kosmos-visuals и сделать сайдбар Delphi использующим компонент из kosmos-visuals.

## Acceptance criteria
- AC1: `packages/kosmos-visuals` contains a reusable Vue sidebar component that encapsulates the Delphi sidebar shell/layout and exposes typed props/slots needed to render primary links, project links, and footer actions without Delphi-specific store logic inside the package.
- AC2: `apps/delphi/ts/src/components/SideBar.vue` is reduced to Delphi-specific composition/orchestration: it derives navigation/project data from the store/router and renders the reusable sidebar component from `@kosmos/visuals` instead of hand-assembling the sidebar UI with `ResizableSidebar` and `SidebarButton` directly.
- AC3: The Delphi sidebar keeps the current behavior: persisted width/hidden state via `delphi-sidebar-config`, macOS toggle button, smart-list links, project list with active highlighting and color dots, settings action, and sidebar hidden-state sync via `setSidebarHidden`.
- AC4: Relevant package/app exports and TypeScript/Vue checks pass for the touched code paths.

## Constraints
- Preserve Delphi visual result and existing routes/labels unless required for the reusable component API.
- Follow Vue 3 Composition API with `<script setup lang="ts">` and typed props/emits.
- Keep reusable UI in `packages/kosmos-visuals`; keep Delphi store access and route-specific derivation in Delphi app code.
- Prefer the smallest safe refactor; do not change unrelated screens or sync logic.

## Non-goals
- Redesigning the Delphi information architecture or changing route destinations.
- Generalizing every possible sidebar variant across the monorepo.
- Adding new navigation features beyond what the current Delphi sidebar already supports.

## Verification plan
- Build: `cd apps/delphi/ts && bunx tsc --noEmit`
- Unit tests: not applicable unless existing tests cover touched files
- Integration tests: not applicable for this refactor
- Lint: `cd apps/delphi/ts && bun run lint`
- Manual checks: inspect `packages/kosmos-visuals` exports and Delphi `SideBar.vue` usage to confirm reusable component wiring and preserved behavior
