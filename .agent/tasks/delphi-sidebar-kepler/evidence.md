# Evidence: delphi-sidebar-kosmos

## AC1: Reusable sidebar component in `packages/kosmos-visuals`

**Status: PASS**

### Proof
- Added `packages/kosmos-visuals/components/Sidebar.vue`.
- The component encapsulates the sidebar shell and visual structure on top of `ResizableSidebar` and `SidebarButton`.
- Public typed contracts are defined in the component itself:
  - `SidebarNavItem`
  - `SidebarProjectItem`
  - `SidebarConfig` passthrough via props/events
- The package component accepts typed props for:
  - `primaryItems`
  - `projectItems`
  - `footerItems`
  - resize/toggle config (`defaultWidth`, `minWidth`, `maxWidth`, `hiddenWidth`, `toggleShortcut`, `dragRegion`, `initialConfig`)
  - macOS toggle presentation (`isMac`, `toggleTitle`)
- Delphi store logic was not moved into the package; the package only renders passed-in data.
- Export wiring added:
  - `packages/kosmos-visuals/components/index.ts`
  - `packages/kosmos-visuals/index.ts`
- Supporting inspection artifact: `.agent/tasks/delphi-sidebar-kosmos/raw/inspection.txt`

## AC2: Delphi sidebar now composes the package component

**Status: PASS**

### Proof
- `apps/delphi/ts/src/components/SideBar.vue` now imports:
  - `Sidebar as KosmosSidebar`
  - `SidebarConfig`
  - `SidebarNavItem`
  - `SidebarProjectItem`
  from `@kosmos/visuals/components`.
- The Delphi component now only:
  - loads/saves persisted sidebar config
  - derives `primaryItems`, `footerItems`, and `projectItems`
  - maps project color tags to CSS classes
  - passes data and config props to `<KosmosSidebar />`
- `SideBar.vue` no longer manually assembles the sidebar markup with `ResizableSidebar`, `SidebarButton`, and inline project `RouterLink`s.
- Supporting inspection artifact: `.agent/tasks/delphi-sidebar-kosmos/raw/inspection.txt`

## AC3: Existing Delphi behavior preserved

**Status: PASS**

### Proof
- Persisted sidebar config remains in `apps/delphi/ts/src/components/SideBar.vue` under `STORAGE_KEY = "delphi-sidebar-config"`.
- Hidden-state sync remains intact via `saveConfig(config)` -> `setSidebarHidden(config.hidden)`.
- The package sidebar still renders:
  - macOS top toggle button with `PanelLeftClose`
  - primary smart-list links
  - project section with active highlighting
  - project color dots
  - footer settings action
- Delphi still passes the same sidebar sizing/shortcut configuration:
  - `default-width="200"`
  - `min-width="160"`
  - `max-width="320"`
  - `hidden-width="80"`
  - `toggle-shortcut="meta+b|ctrl+b"`
  - `drag-region`
- Project active state is preserved through `route.path === "/project/${project.id}"` mapping in Delphi.
- File proof:
  - `packages/kosmos-visuals/components/Sidebar.vue`
  - `apps/delphi/ts/src/components/SideBar.vue`

## AC4: Exports and TypeScript checks

**Status: PASS**

### Proof
- TypeScript check run:
  - Command: `cd /workspace/apps/delphi/ts && ./node_modules/.bin/tsc --noEmit`
  - Result: exit code `0`
  - Artifact: `.agent/tasks/delphi-sidebar-kosmos/raw/build.txt`
- Export wiring present in:
  - `packages/kosmos-visuals/components/index.ts`
  - `packages/kosmos-visuals/index.ts`
- Inspection artifact confirms new exports/import usage:
  - `.agent/tasks/delphi-sidebar-kosmos/raw/inspection.txt`

## Additional verification notes

- `oxlint` could not be executed in this environment because the native optional binding is missing.
  - Artifact: `.agent/tasks/delphi-sidebar-kosmos/raw/lint.txt`
- `oxfmt --check` could not be executed in this environment because the native optional binary package is missing.
  - Artifact: `.agent/tasks/delphi-sidebar-kosmos/raw/format.txt`
- These environment issues did not block AC validation because the acceptance criteria require the touched code paths and exports to compile, and the TypeScript check passed.

## Files changed

- `packages/kosmos-visuals/components/Sidebar.vue`
- `packages/kosmos-visuals/components/index.ts`
- `packages/kosmos-visuals/index.ts`
- `apps/delphi/ts/src/components/SideBar.vue`
