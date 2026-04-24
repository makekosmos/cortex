# Delphi Titlebar Settings And Sidebar Footer

## Original Task Statement

User request summary:

- remove the settings button from the sidebar;
- place settings as a button in the titlebar instead;
- move logbook and trash to the bottom area of the sidebar.

## Summary

Adjust Delphi navigation layout so settings are accessed from the titlebar while the sidebar footer is used for low-frequency destinations (`Журнал`, `Корзина`).

This should be a focused UI rearrangement only. Existing routes and settings screens must continue to work.

## Component Map

- `App.vue`
  - Responsibility: render the new titlebar settings button and navigate to `/settings`.
- `SideBar.vue`
  - Responsibility: keep primary navigation focused on core work views and move `Журнал` / `Корзина` into footer items.

## Acceptance Criteria

- AC1: Delphi no longer renders `Настройки` inside the sidebar footer.
- AC2: Delphi titlebar renders a dedicated settings button that navigates to `/settings`.
- AC3: `Журнал` and `Корзина` are rendered in the bottom/footer area of the sidebar on normal app screens.
- AC4: Existing settings navigation behavior remains intact once on `/settings`.
- AC5: Existing core sidebar items (`Входящие`, `Сегодня`, `Календарь`, `Неделя`) remain in the primary area.
- AC6: Delphi TypeScript remains clean after the change.
- AC7: Delphi production build remains clean after the change.

## Constraints

- Keep the diff focused on Delphi UI composition.
- Do not redesign settings screens themselves.
- Reuse the current router and existing settings route instead of introducing new state.

## Non-Goals

- Settings page redesign.
- Titlebar visual system refactor.
- Shared `kepler-visuals` API changes unless strictly required.

## Verification Plan

1. Run Delphi TypeScript compile check.
2. Run Delphi production build.
3. Inspect the final code to confirm:
   - titlebar button exists in `App.vue`
   - settings removed from sidebar footer
   - logbook and trash are footer items in `SideBar.vue`
