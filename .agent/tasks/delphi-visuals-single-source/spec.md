# Task Spec: delphi-visuals-single-source

## Metadata
- Task ID: delphi-visuals-single-source
- Created: 2026-04-10
- Repo root: /workspace

## Guidance sources
- `/workspace/AGENTS.md`
- `/workspace/apps/delphi/AGENTS.md`
- `/workspace/.agents/skills/vue-best-practices/SKILL.md`

## Original task statement

User reports that `apps/delphi/ts/src/components/SideBarButton.vue` is not identical to `packages/kosmos-visuals/components/SidebarButton.vue` and wants Delphi to stop carrying copied UI components. In this monorepo the app should consume the current shared UI components directly so work happens in one place.

## Component map
- `apps/delphi/ts/src/components/SideBar.vue` — app-specific adapter that maps Delphi store/router data into the shared visual sidebar API.
- `packages/kosmos-visuals/components/*` — single source of truth for shared visual components consumed by Delphi.

## Acceptance criteria

### AC1
Delphi TS must no longer depend on the local duplicate `apps/delphi/ts/src/components/SideBarButton.vue` for runtime behavior; shared sidebar button behavior must come from `packages/kosmos-visuals/components/SidebarButton.vue`.

### AC2
Delphi TS shared-visual imports must go through the shared package public API (`@kosmos/visuals`) instead of deep app-local duplicates or private package component file paths where equivalent root exports already exist.

### AC3
The obsolete duplicate file `apps/delphi/ts/src/components/SideBarButton.vue` must be removed from the app source tree.

### AC4
The Delphi TS app must still build/type-check after the refactor.

## Constraints
- Keep the app-specific sidebar container (`SideBar.vue`) as the composition surface for Delphi-specific store/router mapping.
- Do not change shared component behavior except what is necessary to consume it via the package API.
- Keep diffs minimal and scoped to shared visual component consumption.

## Non-goals
- Renaming Delphi `SideBar.vue` to a different filename.
- Redesigning the sidebar UI.
- Refactoring unrelated Delphi pages.

## Verification plan
- Search imports in `apps/delphi/ts/src` for `@/components/SideBarButton` and private `@kosmos/visuals/components/*.vue` paths.
- Confirm `apps/delphi/ts/src/components/SideBarButton.vue` no longer exists.
- Run `cd /workspace/apps/delphi/ts && ./node_modules/.bin/tsc -p tsconfig.node.json`.
