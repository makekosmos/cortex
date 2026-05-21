# Evidence — Eden task как страница (Pattern B refactor)

**Started**: 2026-05-20T12:35 +0300 (после Pattern C reflexion)
**Completed P1+P2 core (subscribe+NodeView+migration)**: 2026-05-20T13:16 +0300
**Wall-clock**: ~40 мин

## AC

| #    | Criterion                                       | Status               | Evidence                                                                     |
| ---- | ----------------------------------------------- | -------------------- | ---------------------------------------------------------------------------- |
| AC1  | cargo test kepler-backend lib зелёный           | **PASS**             | 141 passed                                                                   |
| AC2  | `bun run --cwd shell build:extensions` зелёный  | **PASS**             | eden bundle 365 KB main + 1.36 MB lazy editor                                |
| AC3  | `bun run --cwd shell typecheck` зелёный         | **PASS**             | exit 0                                                                       |
| AC4  | `bun run ark:guard:writes` зелёный              | **PASS**             | exit 0                                                                       |
| AC5  | `bun run ark:smoke` зелёный                     | **PASS**             | exit 0                                                                       |
| AC6  | `/задача` создаёт task_obj + TaskRef            | **PASS** (по коду)   | Editor.vue slash handler                                                     |
| AC7  | Click на TaskRef → открывает TaskPage           | **PENDING-DEFERRED** | dispatches `eden:open-task` window event, TaskPage сам defer'нут в follow-up |
| AC8  | Toggle checkbox → is_completed update           | **PASS** (по коду)   | TaskRefView.vue toggleCompleted + patchTask                                  |
| AC9  | Toggle is_completed в Delphi → Eden live update | **PASS** (по коду)   | subscribeObjectChanges → loadTask refetch                                    |
| AC10 | Legacy taskItem → taskRef migration on open     | **PASS** (по коду)   | migrateTaskItemsToTaskRefs в hydrateFromEntry                                |
| AC11 | Backspace на TaskRef                            | **PARTIAL**          | удаляет node, task_obj остаётся (UX-выбор: не теряем задачу)                 |

## Файлы

### Rust (foundation)

- `crates/ark-core/rust/src/main.rs` — Request::UpsertObject и DeleteObject теперь эмитят `object_upserted`/`object_deleted` events на успешный write.
- `services/kepler-backend/src/ws_server.rs` — новый tokio select arm подписан на `ark_host.subscribe_events()` и форвардит payload клиентам. Закрыл pre-existing gap: ARK events до этого не доходили до WS клиентов (только command_bus + pomodoro).

### Eden TypeScript

- `extensions/eden/src/TaskRef.ts` — новый atomic block node с командой `insertTaskRef(id)` и VueNodeViewRenderer.
- `extensions/eden/src/components/TaskRefView.vue` — Vue NodeView: чекбокс + title (live из task_obj), single-click (250ms timer) → `eden:open-task` window event, dblclick → inline edit, ARK subscribe для bidir.
- `extensions/eden/src/lib/kepler-api-shim.ts` — добавлены `getTask`, `patchTask`, `createTask`, `subscribeObjectChanges`; удалены Pattern C методы `upsertTaskFromNote` + `EdenTaskSyncInput`; `softDeleteTaskFromNote` переименован в `softDeleteTask`.
- `extensions/eden/src/lib/edenApi.ts` — фасад обновлён.
- `extensions/eden/src/Editor.vue`:
  - Удалены `TaskList`, `TaskItemWithId`, `walkTaskItemsInJson`, `syncTaskItemsToArk`, `lastTaskSnapshot`, `TaskSnapshotEntry` (Pattern C cleanup).
  - Добавлен `TaskRef` в extensions array.
  - Slash «Задача» теперь async: создаёт task_obj через edenApi.createTask, потом insertTaskRef.
  - `migrateTaskItemsToTaskRefs` + интеграция в hydrateFromEntry с baseline-tracking для save flow.
- `extensions/eden/src/TaskItemWithId.ts` — удалён.
- `extensions/eden/package.json` — `@tiptap/core` добавлен (для TaskRef.ts).

## Deferred (follow-up)

- **TaskPage.vue + sidebar integration**: click на TaskRef сейчас dispatch'ит `eden:open-task` window event, но никто его не слушает. UX gap. План: новый TaskPage.vue с полями task_obj (title, description, is_completed, deadline кнопка-открыть-в-Delphi), App.vue ветвление по entry.type_id === "task_obj", расширить listEntries чтобы включать task_obj в sidebar. ~30-45 мин LLM.
- **Backspace soft-delete task_obj**: сейчас Backspace удаляет TaskRef node, task стаётся. Context menu / shortcut «Удалить задачу» — UX дизайн.
- **Rich task fields в TaskPage** (deadline picker, priority, tags, recurrence rule): отдельный proof loop когда UX готов.

## Pattern C → Pattern B miграция

Legacy notes (созданные между 2026-05-20T12:10 и сейчас) содержат TipTap taskItem ноды в content_json + соответствующий task_obj в ARK. Открытие такой ноты в Eden теперь:

1. `migrateTaskItemsToTaskRefs(rawContentJson)` сканирует, заменяет `{type:taskItem, attrs:{taskId, checked}}` на `{type:taskRef, attrs:{taskId}}`. Wrapping taskList развёрнут.
2. baseline = raw (старый) content; new editor content = migrated.
3. queueMicrotask: markDocumentDirty + flushAutoSave → diff видит расхождение → save в ARK.
4. Idempotent: повторное открытие migration уже не делает (нет taskItem'ов).

task_obj не трогаем — он уже создан Pattern C кодом со всеми propsJson (source_app=eden, source_note_id и т.д.). Delphi видит эти задачи без изменений.

## Foundation reusable wins

`ark-core` local emit + ws forward — это не только для Eden ↔ Delphi. Любая cross-app live-реактивность теперь работает:

- Dashboard может live-refresh при изменении usage_session
- Future Olympia/Kerux могут reagiровать на task completion
- Тула observability — каждое write событие видимо

До этого коммита WS клиенты получали ТОЛЬКО `commands_changed` / `command_invoked` / pomodoro events. ark-core events терялись.
