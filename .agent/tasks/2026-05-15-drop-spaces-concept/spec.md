# Drop spaces concept — single DB per user

## Контекст

До 2026-05-15 ARK работал в концепции multi-space: registry
`%APPDATA%\Kosmos\spaces.json` + per-space директории
`%APPDATA%\Kosmos\spaces\<spaceId>\ark.db`. Welcome screen Dashboard'а
показывал список spaces, при клике юзер заходил в space view.

Юзер сформулировал requirement (см. чат 2026-05-15): «у нас теперь вообще
концепции spaces не будет. База данных одна на одного юзера и все».

## Решение

Single ARK DB на юзера: `%APPDATA%\Kosmos\ark.db` (default backend path).
Welcome screen и SpaceCard удалены, Dashboard сразу открывается на единый
список объектов. Backend больше не зависит от `KOSMOS_DB_PATH` /
`selected-space.json`.

LAN sync namespace = глобальный `kepler-default` (фиксированный
`KEPLER_SPACE_ID` в shell ArkClient'е, совпадает с дефолтом backend'а).

## Acceptance criteria

- **AC1**: Dashboard окно открывается сразу на DashboardView (без welcome
  step). Hash `#/dashboard`.
- **AC2**: Backend lock.json показывает `db_path:
C:\Users\<user>\AppData\Roaming\Kosmos\ark.db` (top-level, без `spaces/`).
- **AC3**: Horologion / Delphi / Arrancador видят свои реальные объекты
  (17 игр / 6 time-записей / 4 task'а / 6 заметок) — merge выполнен.
- **AC4**: shell `bun run --cwd shell build:js` PASS.
- **AC5**: shell typecheck PASS.
- **AC6**: `kepler:spaces:list` IPC handler и `window.kepler.spaces` API
  удалены. SpaceMeta помечен @deprecated.
- **AC7**: @kepler/ark selected-space helper'ы помечены @deprecated, но
  остаются функциональными (для legacy/dashboard-extension и mobile/delphi
  миграционных сценариев).

## Что НЕ в scope

- Удаление selected-space helpers из @kepler/ark (только deprecate).
- Refactor mobile/delphi (Kotlin Room, отдельная ContentProvider) — у него
  свои данные, к ARK concept'у spaces не относится.
- Legacy `legacy/dashboard-extension/` (frozen, не трогаем).
- Удаление backup'ов user-data (`%APPDATA%\Kosmos\*.bak-*`) — юзер решит
  сам когда они не нужны.

## Data migration

Выполнено отдельно от commit'ов (runtime action):

```sql
ATTACH DATABASE '<spaces/f028287f78de2d7e/ark.db>' AS src;
BEGIN TRANSACTION;
INSERT OR IGNORE INTO object_types SELECT * FROM src.object_types;
INSERT OR IGNORE INTO objects SELECT * FROM src.objects;
INSERT OR IGNORE INTO object_links SELECT * FROM src.object_links;
COMMIT;
```

Результат: top-level ark.db получил 5 object_types (добавлен `task_obj`)
и 33 объекта (4 заметки top-level + 17 games + 2 notes + 6 time entries +
4 tasks из spaces).

Backup'ы: `ark.db.bak-20260515-010018-pre-merge` и
`spaces/f028287f78de2d7e/ark.db.bak-20260515-010018`.
