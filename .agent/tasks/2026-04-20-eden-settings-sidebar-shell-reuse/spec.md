# Task Spec - Eden settings sidebar shell reuse fix

## Goal
Repair Eden's settings and object types navigation so the app uses one shared outer sidebar shell inside `DesktopChrome` instead of rendering nested sidebars inside the content area.

This fix must restore:
- the same sidebar element across notes, settings, and object types
- preserved sidebar width/hidden state while switching screens
- working exit/back navigation from settings and object types
- object type rows in the sidebar with the actual type icon tinted by the type color
- no mojibake in touched Russian UI strings

## Scope
- `apps/eden/ts/src/App.vue`
- `apps/eden/ts/src/components/sidebar/EdenSidebar.vue`
- `apps/eden/ts/src/components/settings/SettingsPage.vue`
- `apps/eden/ts/src/components/settings/ObjectTypesSettings.vue`

## Acceptance Criteria
- AC1: `DesktopChrome` continues to host exactly one Eden sidebar shell, and that same shell is reused for notes, settings, and object types.
- AC2: Switching between notes, settings, and object types does not inject a second sidebar inside the content area.
- AC3: Sidebar width and hidden state persist when moving between notes, settings, and object types because the same outer shell remains mounted.
- AC4: Settings navigation is available from the shared sidebar, and the user can always navigate back out of settings/object types without getting trapped.
- AC5: Object types mode uses the shared sidebar to list system and custom types, and those rows show the type icon tinted with the type color.
- AC6: Settings and object types main panes remain usable after the sidebar refactor, including opening an existing type and creating a new type draft.
- AC7: Touched Russian UI strings remain UTF-8 and free of mojibake.
- AC8: Eden typecheck and production build pass after the fix.

## Verification Plan
- `bun x tsc --noEmit`
- `bun run build`
- `bun run dev:web`
- targeted UTF-8 spot check on touched Vue files

## Raw Artifact Targets
- `.agent/tasks/2026-04-20-eden-settings-sidebar-shell-reuse/raw/tsc.txt`
- `.agent/tasks/2026-04-20-eden-settings-sidebar-shell-reuse/raw/build.txt`
- `.agent/tasks/2026-04-20-eden-settings-sidebar-shell-reuse/raw/dev-web.txt`
- `.agent/tasks/2026-04-20-eden-settings-sidebar-shell-reuse/raw/utf8-check.txt`
