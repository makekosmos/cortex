# Horologion

Time tracker + pomodoro для Kepler. Toggl Track-стайл без социалки.
Имя приложения — «Horologion» (греч. ὡρολόγιον — часослов). Workspace-директория
исторически остаётся `apps/horologion`. См. `docs-site/apps/horologion.md`
для полной картины.

## Что есть в MVP

- Top bar: «Что я делаю?» + теги (TBD) + проект (TBD) + billable toggle + play/stop.
- Tab **List**: история записей, сгруппированных по дням.
- Tab **Pomodoro**: заглушка.
- **Settings**: заглушка.
- Запись в ARK через `@kepler/ark` → `time_entry_obj`.

## Команды

```powershell
cd apps/horologion
bun run typecheck
bun run dev               # cargo build:sidecar:dev + vite + Electron
bun run build:js          # release sidecar + tsc + vite build (без установщика)
bun run build             # build:js + electron-builder --win msi (финальный MSI)
```

## Архитектура

- **Renderer (Vue)**: UI, локальный state таймера. Не имеет доступа к SQLite.
- **Electron main**: `ArkClient` self-managed sidecar, IPC handlers для time entries и tags.
- **Preload**: типизированный `window.horologion` API (см. `shared/ipc-types.ts`).

ARK объекты:

- `time_entry_obj` — запись (start, end, kind, billable). Тип seeded в `ark-core`.
- `tag_obj` — общий с Delphi. Связь через `object_links` (`linkType='tagged'`).
- `task_obj` (Delphi) — связь через `object_links` (`linkType='for-task'`).

## TODO (приоритет)

1. Тег-пикер в top bar (создание/выбор `tag_obj`, object_link к time_entry).
2. `@mention` autocomplete по Delphi `task_obj` → `linkType='for-task'`.
3. Pomodoro implementation (25/5/15, авто time_entry per segment).
4. Settings UI (длительности, дефолтные теги).
5. Calendar view.
6. Импорт из `usage-tracker` (предложить time_entry из foreground-периодов).
