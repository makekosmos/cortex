# 2026-05-28 — Focus widget stability and unified controls

## Цель

Убрать визуальное дёргание Pomodoro focus widget при активной задаче и привести базовый виджет к единому mini-player стилю на `@kosmos/visuals`: в покое показываются только время и название, действия появляются при hover/focus.

## Scope

- `shell/electron/focus-widget.ts`: стабильность label при конкурентных state updates от backend и Horologion renderer.
- `shell/src/views/FocusWidgetView.vue`: компактный UI, controls только на hover/focus, сохранение доступности кнопок.
- Регрессия в существующих focus-widget e2e тестах.
- Постмортем в `docs-site/agents/postmortems.md`.

## Out Of Scope

- Изменение pomodoro lifecycle в backend.
- Изменение ARK schema или write-boundary.
- Новый дизайн Horologion основного окна.
- Изменение размеров/позиционирования BrowserWindow за пределами необходимости для layout.

## Acceptance Criteria

**AC1.** Backend/main-process update не перетирает непустой label виджета дефолтным `Фокус`/`Перерыв`, если renderer уже передал конкретное название задачи или сессии.

**AC2.** В базовом состоянии focus widget показывает только countdown и label; кнопки pause/resume, skip, stop и close не занимают видимое пространство до hover/focus внутри виджета.

**AC3.** Inline controls остаются доступны по hover/focus и сохраняют существующие действия/aria-labels для pomodoro и stopwatch modes.

**AC4.** Существующие focus-widget проверки проходят в headless mode на изолированной БД.
