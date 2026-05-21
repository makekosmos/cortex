# Task Spec — Eden sidebar migration to @kosmos/visuals

## Source of truth

- `.omx/plans/prd-eden-sidebar-kosmos-visuals.md`
- `.omx/plans/test-spec-eden-sidebar-kosmos-visuals.md`
- `.omx/context/eden-sidebar-kosmos-visuals-20260415T001900Z.md`

## Goal

Перевести Eden main/settings sidebar surfaces на shared `@kosmos/visuals` через public API, убрать spaces navigation из main UI, перенести выбор пространства в Settings и привести macOS sidebar head spacing к паттерну Delphi.

## Acceptance Criteria

- AC1: Main sidebar Eden использует shared `Sidebar` из `@kosmos/visuals`.
- AC2: Settings page использует shared `Sidebar` из `@kosmos/visuals`.
- AC3: В main UI больше нет отдельной spaces sidebar/surface.
- AC4: Выбор пространства доступен в Settings.
- AC5: На macOS верх sidebar shell корректно оставляет место под traffic lights.
- AC6: Не деградируют сценарии open note / create note / search / open settings / change space / return to work screen.
- AC7: Eden импортирует sidebar only via `@kosmos/visuals` public API.

## Bounded implementation plan

1. Расширить shared sidebar contract для non-router/button-driven items без поломки Delphi.
2. Добавить Eden adapter components for main/settings sidebars using shared `Sidebar`.
3. Убрать spaces items from main sidebar; добавить `SpacesSettings` tab.
4. Перевести `App.vue` и `SettingsPage.vue` на adapters.
5. Добавить regression tests for shared main/settings sidebar and space selection.
6. Verify via lint/build/e2e.
