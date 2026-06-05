# Focus launcher commands (state-aware)

## Контекст

Лаунчер показывает команду **«Начать фокус»** (`kepler:focus-session`) всегда. Во
время активной фокус-сессии она бессмысленна, а управление сессией доступно только
через floating widget. Нужно: во время активной сессии заменить «Начать фокус» на
набор команд, идентичный виджету.

Также чинится потеря клавиатурного фокуса в лаунчере (отдельный коммит, task 1).

## Acceptance criteria

1. **Idle (нет активной сессии):** в списке команд видна «Начать фокус». Сырые
   command-bus focus-команды (`kepler:focus-toggle/pause/resume/skip/complete`) в
   лаунчере НЕ показываются (их заменяет состояние-зависимый набор).
2. **Active running:** «Начать фокус» скрыта; вместо неё в списке:
   - **Пауза** → `focusSession.pause()`
   - **Выполнена** → stop + пометить привязанную Delphi-задачу done
   - **Завершить** → `focusSession.stop()`
   - **Редактировать** → открыть focus-панель (edit mode)
3. **Active paused:** команда паузы показывает заголовок **Продолжить** →
   `focusSession.resume()`.
4. **Редактировать:** открывает почти ту же панель (предзаполнена текущей сессией),
   но кнопка футера называется **Продолжить** вместо **Начать фокус**.
5. «Выполнена» помечает `task_obj` (props `status:"done"`, `is_completed:true`,
   `completed_at`) через `upsert_object` (тот же write-path, что и time_entry —
   sync version bump в backend). Если задача не привязана — просто stop.
6. Список реагирует на смену состояния сессии (старт/пауза/стоп из виджета),
   подписка на `focusSession.onUpdated`.
7. Pause/resume/done/stop из лаунчера не закрывают лаунчер (видна смена списка);
   «Редактировать» переводит в focus-режим.

## Дизайн

- Pure helper `shell/src/lib/focusLauncherCommands.ts`:
  `buildFocusAwareCommands(commands, { active, paused })` + id-константы. Покрыт
  unit-тестами (`tests/unit/focus-launcher-commands.test.ts`).
- `LauncherView.vue`: `focusSnapshot` reactive + `displayCommands` computed;
  `invokeSelected` спец-кейсы для синтетических id; `focusEditMode` для футера.
- Backend `focus-session.ts`: `completeFocusSession()` (stop + markTaskDone) +
  IPC `kepler:focus-session:complete`; preload + ipc-types.

## Проверки

- `bun test tests/unit/focus-launcher-commands.test.ts` (RED → GREEN)
- `bun run shell:typecheck`
- `bun run ark:guard:writes` (затронут data-path: markTaskDone)
- Visual verify focus-режима (скрин под `.tmp/`) — отложено до ручного теста юзером.
