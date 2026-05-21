# Drop spaces concept — evidence

## Pre-state

`%APPDATA%\Kosmos\` (2026-05-15 00:30):

- `ark.db` (top-level) — 22.91 MB. objects: 4 note_obj. usage_sessions:
  10018, usage_events: 26734, tracked_apps: 103. Legacy Delphi tables:
  todos / areas / headings / projects / tags (заполнены до Phase 4
  migration).
- `spaces.json` — registry: Personal (DCE9X21A8HYT), MWVQ-YBRE-WTQK.
- `selected-space.json` — указывал на Personal (spaceId
  `f028287f78de2d7e`).
- `spaces\f028287f78de2d7e\ark.db` — 462 KB. objects: 17 game_obj + 6
  time_entry_obj + 4 task_obj + 2 note_obj = 29.
- `spaces\2b42c913678b1572\ark.db` — 4 KB, no `objects` table (MWVQ
  никогда не использовался).
- `spaces\76a639248aa6ea4d\`, `spaces\main\` — orphan dirs без ark.db.

## Actions

### 1. Backup

```powershell
Copy-Item %APPDATA%\Kosmos\ark.db{,.bak-20260515-010018-pre-merge}
Copy-Item %APPDATA%\Kosmos\ark.db-shm{,.bak-20260515-010018-pre-merge}
Copy-Item %APPDATA%\Kosmos\ark.db-wal{,.bak-20260515-010018-pre-merge}
Copy-Item %APPDATA%\Kosmos\spaces\f028287f78de2d7e\ark.db `
  %APPDATA%\Kosmos\spaces\f028287f78de2d7e\ark.db.bak-20260515-010018
```

### 2. WAL checkpoint + merge

```sql
PRAGMA wal_checkpoint(TRUNCATE);  -- top-level
ATTACH DATABASE
  'C:/Users/Kazui/AppData/Roaming/Kosmos/spaces/f028287f78de2d7e/ark.db'
  AS src;
BEGIN TRANSACTION;
INSERT OR IGNORE INTO object_types SELECT * FROM src.object_types;
INSERT OR IGNORE INTO objects SELECT * FROM src.objects;
INSERT OR IGNORE INTO object_links SELECT * FROM src.object_links;
COMMIT;
DETACH DATABASE src;
```

Post-merge counts top-level ark.db:

- object_types: 5 (game_obj, note_obj, tag_obj, task_obj, time_entry_obj)
- objects: 33 (17 game_obj + 6 note_obj + 6 time_entry_obj + 4 task_obj)
- usage_sessions: 10018 (сохранены)

### 3. Code refactor

Коммиты:

- `0c4c715 feat(shell)!: drop spaces concept — single DB per user`
- `e70a4c0 chore(@kepler/ark): mark selected-space helpers @deprecated`

См. `git show 0c4c715 e70a4c0`.

### 4. Archive user-data

```powershell
Move-Item %APPDATA%\Kosmos\spaces.json{,.bak-20260515-010018}
Move-Item %APPDATA%\Kosmos\selected-space.json{,.bak-20260515-010018}
Rename-Item %APPDATA%\Kosmos\spaces spaces.bak-20260515-010018
```

## AC results

- **AC1** (Dashboard сразу открывает DashboardView без welcome): PASS —
  `dashboard-window.ts` грузит hash `#/dashboard`, `DashboardRoot.vue`
  безусловно рендерит `<DashboardView />`. См. `0c4c715`.
- **AC2** (backend lock.json db_path top-level): pending real-run
  verification после следующего `bun run --cwd shell dev` (юзер должен
  перезапустить — старый backend killed pre-merge).
- **AC3** (apps видят свои объекты): pending real-run verification.
- **AC4** (shell build): PASS — `bun run --cwd shell build:js` clean
  (см. terminal output после commit 0c4c715).
- **AC5** (shell typecheck): PASS — `tsc --noEmit` clean после `0c4c715`.
- **AC6** (kepler:spaces:list / window.kepler.spaces удалены): PASS —
  grep `kepler:spaces|window\.kepler\.spaces` в shell/ возвращает 0
  результатов после commit'а.
- **AC7** (@kepler/ark selected-space helpers @deprecated): PASS — см.
  `e70a4c0`.

## Open verification

AC2 и AC3 требуют запуск `bun run --cwd shell dev` со стороны юзера
(детальный pid + DB inspection в живом backend'е). После запуска юзер
сообщит результат.
