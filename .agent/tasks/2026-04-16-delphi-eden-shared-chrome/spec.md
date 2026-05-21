# Task: Delphi and Eden shared chrome

## Goal

Adopt the shared `@kosmos/visuals` desktop titlebar and shared content surface in both `apps/delphi/ts` and `apps/eden/ts`, so each app uses the same top chrome pattern as dashboard: titlebar above sidebar and a bordered content surface under it.

## Component Map

- `packages/kosmos-visuals/DesktopChrome`: shared shell that hosts titlebar, sidebar, and content area.
- `packages/kosmos-visuals/DesktopContentSurface`: shared bordered content pane reused by dashboard, Delphi, and Eden.
- `apps/delphi/ts/src/App.vue`: composition surface wiring shared chrome, Delphi sidebar, top status/actions, and routed main content.
- `apps/eden/ts/src/App.vue`: composition surface wiring shared chrome, Eden sidebar, titlebar actions, and existing screens.

## Acceptance Criteria

- AC1: `apps/delphi/ts` uses the shared titlebar above the sidebar and wraps routed content in the shared content surface.
- AC2: `apps/eden/ts` uses the shared titlebar above the sidebar and wraps main app content in the shared content surface.
- AC3: Both apps keep their sidebar behavior and hidden-state flows working after the chrome swap.
- AC4: Electron window configuration in both apps is compatible with the shared titlebar layout.
- AC5: Relevant AGENTS instructions are updated for touched apps.
- AC6: TypeScript/Vite verification passes for `apps/delphi/ts` and `apps/eden/ts`.
