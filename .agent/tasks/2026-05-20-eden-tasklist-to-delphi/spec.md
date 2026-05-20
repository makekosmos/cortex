# Eden TaskList → Delphi sync

**Date**: 2026-05-20
**Slug**: 2026-05-20-eden-tasklist-to-delphi
**Status**: in-progress

## Goal

В Eden TipTap editor добавить TaskList/TaskItem (чекбокс-задачи). Каждый
taskItem параллельно представляется как `task_obj` в ARK — то есть появляется
в Delphi (и в Dashboard) автоматически. MVP — одностороннее Eden → Delphi.

## Why

`task_obj` уже — общий объект экосистемы (см. CLAUDE.md: «всё есть объект»).
Eden сейчас умеет работать только с `note_obj`. Юзер часто пишет «надо сделать
X» внутри заметки, и хочет чтобы это автоматически попало в Delphi список — без
двойного ручного ввода.

## Scope

### IN

1. `@tiptap/extension-task-list` + `@tiptap/extension-task-item` подключены в
   `extensions/eden/src/Editor.vue`.
2. Кастомный `TaskItemWithId` с атрибутом `taskId` (UUID v4), генерируется
   автоматически при создании taskItem без id (через `addOptions` /
   `addStorage` или onCreate hook).
3. Slash-команда `/задача` (рус.) → `chain().toggleTaskList().run()`.
4. Eager регистрация `task_obj` object_type на boot Eden extension'а
   (`extensions/eden/src/main.ts` или App.vue) — аналогично Delphi
   `ensureTaskObjectTypeRegistered`.
5. `kepler-api-shim.ts` + `edenApi.ts` получают методы:
   - `ensureTaskObjectTypeRegistered()`
   - `upsertTaskFromNote(taskId, title, isCompleted, sourceNoteId)`
   - `softDeleteTask(taskId)`
6. После каждого успешного save заметки — diff taskItem'ов:
   - **новые** → `upsertTaskFromNote(...)` (создание task_obj)
   - **изменённые** (title или checked) → `upsertTaskFromNote(...)`
   - **исчезнувшие** → `softDeleteTask(taskId)`
7. Snapshot предыдущих taskId+title+checked хранится in-memory в Editor
   instance (`lastTaskSnapshot: Map<taskId, {title, checked}>`).
8. Markdown сериализация — стандартная `- [ ]` / `- [x]` через
   `@tiptap/markdown`. `taskId` сохраняется только в `content_json` (для md
   это extra attr — невидим, ОК).

### OUT (отдельные задачи / future)

- ❌ Bidirectional sync (Delphi → Eden): если Delphi меняет task, Eden
  отображает stale checkbox до перезагрузки заметки.
- ❌ UI для свойств задачи (deadline, scheduledDate, tags) изнутри TipTap.
  В Eden task_obj создаётся с минимумом: title + isCompleted + sourceNoteId.
- ❌ Восстановление task_obj если удалена в Delphi, а строка в Eden осталась.
- ❌ E2e Playwright spec — manual smoke достаточно для MVP.

## Acceptance Criteria

| # | Criterion | How to verify |
|---|---|---|
| AC1 | `bun run --cwd extensions/eden build` зелёный | `echo $?` |
| AC2 | `bun run --cwd extensions/eden typecheck` зелёный | `echo $?` |
| AC3 | `bun run ark:guard:writes` зелёный (новый write-path идёт через `@kosmos/ark` shim, нет direct SQL) | guard exit 0 |
| AC4 | `bun run ark:smoke` зелёный | smoke output |
| AC5 | В dev Eden: ввести `/задача` → появляется чекбокс-список | manual screenshot |
| AC6 | После save заметки с 2 task'ами → в Dashboard видны 2 объекта типа `task_obj` с правильным title | manual через Settings → Dashboard или прямой `list_objects_by_type task_obj` |
| AC7 | Toggle чекбокса в Eden + save → `is_completed` в task_obj обновляется | manual через Dashboard |
| AC8 | Удалить строку taskItem в Eden + save → объект помечен `deletedAt != null`, исчезает из Delphi | manual через Delphi UI |
| AC9 | `task_obj` создан с `propsJson.source_app = "eden"` и `propsJson.source_note_id = <entry.id>` | inspect через Dashboard детали объекта |
| AC10 | Sync failure (например искусственно сломать shim) НЕ блокирует note save — заметка сохраняется, в console warn | manual repro |

## Risks

- **R1: ID instability при copy-paste**. taskItem копируется с тем же taskId →
  дубликат в редакторе. **Mitigation**: в `onCreate`/`parseHTML` taskItem
  смотрит — если уже есть taskItem с этим id в этом доке, regenerate.
  → MVP: оставляем как есть (юзер вряд ли копипастит, и `upsert_object` всё
  равно идемпотентен по id — обновит существующий task_obj). Документируем
  как known limitation.
- **R2: Гонка с autosave debounce**. Если юзер быстро правит, save может
  ещё не завершиться к моменту следующего diff. **Mitigation**: sync
  выполняется в `flushAutoSave` сразу **после** успешного `onSave(...)`
  resolve, в той же async-функции. Не параллельный таймер.
- **R3: task_obj sync падает** (network / ARK недоступен). **Mitigation**:
  обернуть в try/catch, console.warn, **не** throw → не блокирует note save.
  Snapshot обновляется только на successful sync, чтобы при следующем save
  ретрай прошёл по тому же дельте.

## Plan

1. Spec — этот файл (✓).
2. Deps + shim/edenApi методы.
3. Eager registration в Eden main.ts.
4. TaskItemWithId extension + slash command.
5. Walk + diff + sync wiring в Editor.vue save flow.
6. typecheck + build + guards.
7. Manual smoke (создать заметку → Dashboard → Delphi).
8. evidence.md AC=PASS.

## Out of scope justification

Bidirectional sync — отдельная фича, требует subscription на ARK events
+ TipTap transaction для обновления чекбокса без autosave loop. Не входит
в MVP.
