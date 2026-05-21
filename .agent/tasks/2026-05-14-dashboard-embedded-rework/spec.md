# Dashboard → embedded ARK browser (rework)

**Date**: 2026-05-14
**Slug**: dashboard-embedded-rework
**Status**: in-progress

## Context

Dashboard сейчас живёт как Vue extension в `extensions/dashboard/`. По решению пользователя
Dashboard становится встроенной частью Kepler shell'а — entry point в данные ARK с двумя
вьюхами:

1. **Welcome (space picker)** — карточки spaces в стиле iCloud.
2. **Space view** — sidebar (Всё / Настройки / object_types) + main pane (таблица объектов).

Старый extension'овский dashboard замораживается в `legacy/dashboard-extension/`.

## Цель

Внутри shell render bundle добавить routing `#/dashboard/...`, новый `DashboardWelcomeView`

- `DashboardSpaceView`, отдельное окно через `electron/dashboard-window.ts`, IPC `kepler:spaces:*`
  для перечисления spaces. Старый extension Dashboard — frozen в legacy/.

## Acceptance criteria

- **AC1**: Tray menu имеет «Dashboard» item; click открывает welcome window (~1200×800).
- **AC2**: Welcome показывает sphere logo, «Kosmos» текст, минимум 1 space card (current selected
  space из `selected-space.json` или из `<APPDATA>/Kosmos/spaces/` dir scan).
- **AC3**: Click space card → переход в space view (hash route changes).
- **AC4**: Sidebar показывает «Всё», «Настройки», группу «Типы» с реальными object_types
  (через `list_object_types`).
- **AC5**: Click sidebar тип → main pane показывает таблицу объектов того type'а (через
  ARK query `list_objects_by_type`).
- **AC6**: Columns «Значение», «Тип», «Добавлено», «Данные X», «Данные Y» — наполнены
  реальными данными из object.contentJson / propsJson или sensible fallback («—»).
- **AC7**: `bun run --cwd shell build:js` PASS, typecheck PASS.
- **AC8**: Закрытие dashboard окна **не** закрывает Kepler shell (main launcher продолжает работать).
- **AC9**: `bun run docs:check` 0 stale.
- **AC10**: `extensions/dashboard/` исчез (frozen в `legacy/dashboard-extension`), main launcher
  всё ещё работает; old `dashboard:open` команда либо удалена, либо переведена на новый
  `kepler:dashboard:open` (open static command).

## Не делать

- НЕ удалять `extensions/dashboard/` полностью (`git mv` в legacy/).
- НЕ ломать другие extensions (delphi, horologion, arrancador).
- НЕ создавать новые ARK endpoints (использовать existing).
- НЕ пушить.
- НЕ trogать `extension-host.ts` сильно.
- НЕ ломать `Ctrl+Shift+K` launcher.

## Атомарные commits

- **C1**: `refactor: freeze extensions/dashboard → legacy/`
- **C2**: `feat(dashboard): scaffold welcome view + dashboard window + tray entry`
- **C3**: `feat(dashboard): spaces preload API + listSpaces handler`
- **C4**: `feat(dashboard): space view с sidebar + object table`
- **C5**: `docs: rewrite dashboard.md под embedded архитектуру`
