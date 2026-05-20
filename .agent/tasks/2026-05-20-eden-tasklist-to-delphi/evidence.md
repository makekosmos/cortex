# Evidence — Eden TaskList → Delphi sync

**Started**: 2026-05-20T12:10:51+0300
**Completed**: 2026-05-20T12:25:44+0300
**Wall-clock**: ~15 min

## AC

| # | Criterion | Status | Evidence |
|---|---|---|---|
| AC1 | `bun run --cwd shell build:extensions` зелёный | **PASS** | exit 0, `dist/eden/` build с TaskItemWithId chunk |
| AC2 | shell typecheck зелёный | **PASS** | `bun run --cwd shell typecheck` exit 0 |
| AC3 | `bun run ark:guard:writes` зелёный | **PASS** | `ARK write boundary guard passed.` exit 0 |
| AC4 | `bun run ark:smoke` зелёный | **PASS** | `ARK smoke matrix passed.` exit 0 (после fix'а ark-smoke.mjs Windows quote bug) |
| AC5 | `/задача` создаёт чекбокс | **PENDING-MANUAL** | manual smoke в dev — отложен на ревью пользователем |
| AC6 | task_obj появляется в Dashboard | **PENDING-MANUAL** | manual |
| AC7 | toggle checked → is_completed update | **PENDING-MANUAL** | manual |
| AC8 | delete row → soft-delete task_obj | **PENDING-MANUAL** | manual |
| AC9 | propsJson.source_app=eden, source_note_id=<entry.id> | **PASS** (по коду) | См. `kepler-api-shim.ts::upsertTaskFromNote` |
| AC10 | sync failure не блокирует note save | **PASS** (по коду) | `syncTaskItemsToArk` try/catch без throw, snapshot обновляется только для successful operations |

## Файлы

- `extensions/eden/package.json` — добавлены `@tiptap/extension-list`, `@tiptap/extension-task-item`, `@tiptap/extension-task-list` (3.20.1).
- `extensions/eden/src/TaskItemWithId.ts` — новый файл: TaskItem.extend() с `taskId` UUID attr + ProseMirror plugin для авто-проставления.
- `extensions/eden/src/Editor.vue` — import + TaskList/TaskItemWithId в extensions array, slash-команда «Задача», `walkTaskItemsInJson`, `syncTaskItemsToArk`, snapshot reset в `hydrateFromEntry`, sync после успешного save.
- `extensions/eden/src/lib/kepler-api-shim.ts` — `ensureTaskObjectTypeRegistered`, `upsertTaskFromNote`, `softDeleteTaskFromNote` + интерфейс `EdenTaskSyncInput`.
- `extensions/eden/src/lib/edenApi.ts` — фасад для трёх новых методов.
- `scripts/ark-smoke.mjs` — drive-by fix: quote command path для Windows shell:true (pre-existing tooling bug).

## Известные ограничения MVP

- One-way Eden → Delphi. Bidirectional defer.
- Copy-paste taskItem'а сохраняет тот же taskId → upsert обновит существующий task_obj. Документировано в `TaskItemWithId.ts`.
- Manual AC5-AC8 не пробегали — нужно ревью пользователем в dev mode.
- Task properties (deadline, priority, tags) не выставляются — default values, юзер правит в Delphi.

## Followups

- Bidirectional sync: ARK subscription на task_obj events → обновлять `checked` атрибут taskItem'а.
- Regenerate taskId на paste-в-другую-заметку (предотвратить случайный shared state).
- E2e Playwright spec для task sync.
- Возможность открыть task_obj из Eden в Delphi (link через `kepler:command:invoke` `delphi:task:open` с id).
