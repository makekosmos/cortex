# Delphi Settings Sidebar Reuse

## Goal

Use the same Delphi `SideBar.vue` component for both normal app navigation and the `/settings` route, changing only the button set instead of rendering a different sidebar implementation.

## Acceptance Criteria

- AC1: `apps/delphi/ts/src/components/SideBar.vue` is the single sidebar component used on both normal routes and `/settings`.
- AC2: On `/settings`, the sidebar shows settings-specific actions (`Назад`, `Общие`, `Пространства`) using the same sidebar component and sizing persistence as the normal app sidebar.
- AC3: `apps/delphi/ts/src/pages/SettingsPage.vue` no longer renders its own sidebar or its own `DesktopContentSurface`; it renders only settings content.
- AC4: `apps/delphi/ts/src/App.vue` uses one shared sidebar slot and one shared `DesktopContentSurface` for all routes, including `/settings`.
- AC5: Settings tab selection still uses the route query and remains stable through refresh/back.
- AC6: `bunx tsc --noEmit` passes in `apps/delphi/ts`.
