# Evidence: delphi-visuals-single-source

## Summary
Removed the stale local `SideBarButton` copy from Delphi TS and pointed Delphi shared visual imports at the workspace package public API so shared UI work happens in `@kosmos/visuals`.

## Acceptance criteria

### AC1 — PASS
Delphi TS no longer depends on the local duplicate `apps/delphi/ts/src/components/SideBarButton.vue` for runtime behavior.

Proof:
- No app imports remain for `SideBarButton.vue` or `@kosmos/visuals/components` deep component paths in Delphi TS source.
- Shared sidebar rendering in `apps/delphi/ts/src/components/SideBar.vue` now imports `Sidebar` and related types from `@kosmos/visuals`.
- Raw artifacts:
  - `.agent/tasks/delphi-visuals-single-source/raw/import-audit.txt`
  - `.agent/tasks/delphi-visuals-single-source/raw/diff.txt`

### AC2 — PASS
Delphi TS shared visuals now go through the shared package public API (`@kosmos/visuals`) where equivalent root exports exist.

Proof:
- Updated imports:
  - `apps/delphi/ts/src/App.vue`
  - `apps/delphi/ts/src/components/QuickSearch.vue`
  - `apps/delphi/ts/src/components/SideBar.vue`
  - `apps/delphi/ts/src/pages/AllTaskPage.vue`
  - `apps/delphi/ts/src/pages/LogbookPage.vue`
  - `apps/delphi/ts/src/pages/ProjectPage.vue`
  - `apps/delphi/ts/src/pages/TodayPage.vue`
  - `apps/delphi/ts/src/pages/TrashPage.vue`
- Audit output shows Delphi TS imports shared visuals from `@kosmos/visuals`.
- Raw artifact: `.agent/tasks/delphi-visuals-single-source/raw/import-audit.txt`

### AC3 — PASS
The obsolete duplicate file `apps/delphi/ts/src/components/SideBarButton.vue` has been removed.

Proof:
- File absence check wrote `missing` to `.agent/tasks/delphi-visuals-single-source/raw/sidebar-button-file.txt`.
- Raw artifact: `.agent/tasks/delphi-visuals-single-source/raw/sidebar-button-file.txt`

### AC4 — PASS
The Delphi TS app still passes the available TypeScript check after the refactor.

Proof:
- Command: `cd /workspace/apps/delphi/ts && ./node_modules/.bin/tsc -p tsconfig.node.json`
- Exit code: `0`
- Raw artifacts:
  - `.agent/tasks/delphi-visuals-single-source/raw-tsc.meta`
  - `.agent/tasks/delphi-visuals-single-source/raw-tsc.txt`

## Commands run
- `cd /workspace && rg -n '@kosmos/visuals/components|SideBarButton\.vue|from "@kosmos/visuals"|from "@kosmos/visuals/components"' apps/delphi/ts/src --glob '!**/node_modules/**'`
- `test ! -e /workspace/apps/delphi/ts/src/components/SideBarButton.vue`
- `cd /workspace/apps/delphi/ts && ./node_modules/.bin/tsc -p tsconfig.node.json`
- `cd /workspace && git diff -- apps/delphi/ts/src/App.vue apps/delphi/ts/src/components/QuickSearch.vue apps/delphi/ts/src/components/SideBar.vue apps/delphi/ts/src/pages/TodayPage.vue apps/delphi/ts/src/pages/AllTaskPage.vue apps/delphi/ts/src/pages/LogbookPage.vue apps/delphi/ts/src/pages/ProjectPage.vue apps/delphi/ts/src/pages/TrashPage.vue apps/delphi/ts/src/components/SideBarButton.vue apps/delphi/AGENTS.md`
