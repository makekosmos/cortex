# Delphi Settings Shared Shell

## Goal

Align the Delphi settings screen with the normal Delphi shell by using the same shared sidebar/content primitives and the same sidebar sizing profile as the regular app sidebar.

## Acceptance Criteria

- AC1: The `/settings` route owns its own shell built from shared `@kepler/visuals` components: a sidebar on the left and `DesktopContentSurface` for the content area.
- AC2: The settings sidebar uses the same width sizing profile as the regular Delphi sidebar (`default 200`, `min 160`, `max 320`).
- AC3: The outer app shell in `App.vue` does not wrap `/settings` in the normal app `DesktopContentSurface`, avoiding nested surfaces.
- AC4: Settings navigation still contains `Назад`, `Общие`, and `Пространства`, and tab selection remains stable through the route query.
- AC5: `bunx tsc --noEmit` passes in `apps/delphi/ts`.
