# Eden: task как страница (Pattern B refactor)

**Date**: 2026-05-20
**Slug**: 2026-05-20-eden-task-as-page
**Status**: in-progress
**Replaces**: 2026-05-20-eden-tasklist-to-delphi (Pattern C stopgap)

## Goal

Task в Eden — это **отдельная страница** (как заметка): видна в sidebar, открывается в main view, имеет full editor с полями. В заметке task представлена как `TaskRef` node (Vue NodeView), который **рендерит checkbox + title из `task_obj` live**, без дублирования данных. Single click → navigate to task page. Double click → inline edit title.

Bidir sync исчезает как проблема: один canonical UI (task page), Eden NodeView и Delphi list — это **views** одного объекта.

## Why

Pattern C (наша реализация час назад) — два source of truth: title живёт в `note_obj.content_json` taskItem'ах и в `task_obj.title`. Любое расхождение требует sync кода. Это противоречит Kosmos north star ([[project_north_star_object_graph]]: «typed objects + cross-type links; markdown — рендеринг для людей, не source of truth»).

Pattern B (объект первичен, рендер вторичен) согласован с Anytype-моделью, которая и есть Kosmos north star.

## Scope

### IN

1. **ark-core local emit**: `crates/ark-core/rust/src/main.rs` эмитит `entity_changed` event на локальные `upsert_object`/`delete_object`, не только на sync-incoming. Backwards compatible (consumer'ы уже его обрабатывают для sync).
2. **TaskRef TipTap node**: новый Vue NodeView `extensions/eden/src/TaskRef.ts` + `TaskRefView.vue`.
   - Attrs: `taskId: string`
   - Renders: checkbox + title (live из task_obj)
   - Subscribes ARK `entity_changed` фильтр typeId="task_obj", id=taskId
   - Single click (с 250ms timer) → eden.navigateTo(taskId)
   - Double click → contenteditable focus title input
   - Backspace на пустой taskRef → удаляет node + soft-delete task_obj
3. **TaskPage.vue**: full editor task'а в main view.
   - Title (large input)
   - Description (плейн textarea для MVP, без TipTap)
   - is_completed checkbox
   - Кнопка "Открыть в Delphi" (command bus invoke) — для rich-полей (deadline / priority / tags), которые пока полно правятся только там
   - Save через `edenApi.upsertTaskPage(task)` — то же `upsert_object` но typed shape
4. **App.vue ветвление**: когда `currentEntry.type_id === "task_obj"` → `<TaskPage>`, иначе `<Editor>`.
5. **Sidebar**: `listEntries()` в shim'е расширяется — fetch'ит и note_obj и task_obj, отдаёт в общем списке. Sidebar group'ирует по type_id (как сейчас типизированные заметки).
6. **Slash `/задача`**: переключается на создание `taskRef` — сначала `upsert_object` task_obj, потом insert taskRef node с этим id.
7. **Migration**: при hydrate ноты — если есть `taskItem` ноды с `taskId` (Pattern C legacy), конвертируем в `taskRef` в content_json, стрипим inline text. Идемпотентно. Save после миграции.
8. **Cleanup**: удалить `walkTaskItemsInJson`, `syncTaskItemsToArk`, `lastTaskSnapshot`, `TaskItemWithId.ts`, slash-команду «Задача» которая создаёт taskItem. Оставить только taskRef flow.

### OUT

- ❌ Rich fields UI для deadline / priority / tags / recurrence / checklist в TaskPage. Эти поля редактируются в Delphi (кнопка «Открыть в Delphi» в TaskPage).
- ❌ `- [ ]` markdown shortcut (только slash на MVP).
- ❌ Сложный conflict UI при concurrent edit. HLC last-writer-wins на task_obj.
- ❌ Реактивность через CRDT.

## Acceptance Criteria

| #    | Criterion                                                                                           | How to verify                 |
| ---- | --------------------------------------------------------------------------------------------------- | ----------------------------- |
| AC1  | `cargo test --manifest-path services/kepler-backend/Cargo.toml --lib` зелёный после ark-core change | cargo test exit 0             |
| AC2  | `bun run --cwd shell build:extensions` зелёный                                                      | exit 0                        |
| AC3  | `bun run --cwd shell typecheck` зелёный                                                             | exit 0                        |
| AC4  | `bun run ark:guard:writes` зелёный                                                                  | exit 0                        |
| AC5  | `bun run ark:smoke` зелёный                                                                         | exit 0                        |
| AC6  | `/задача` в Eden → создаёт task_obj + TaskRef node; checkbox + title виден                          | manual                        |
| AC7  | Click на TaskRef → открывает TaskPage с полями                                                      | manual                        |
| AC8  | Toggle checkbox в TaskPage / в Eden TaskRef → is_completed update в task_obj                        | manual via Dashboard          |
| AC9  | Toggle is_completed в Delphi → TaskRef checkbox в Eden обновляется **live**                         | manual                        |
| AC10 | Старая нота с taskItem (Pattern C) при открытии конвертируется в taskRef                            | inspect content_json до/после |
| AC11 | Backspace на пустом TaskRef → soft-delete task_obj                                                  | manual + Dashboard inspect    |

## Risks

- **R1: ark-core local emit ломает sync** — sync replication уже эмитит entity_changed; если локальный upsert тоже эмитит, то peer'овский accept'ed sync write эмитит дважды (один раз sync-side, один раз local через RPC). **Mitigation**: добавлять local emit ТОЛЬКО в `Request::UpsertObject` handler в main.rs (не в sync code path).
- **R2: NodeView перформанс при 50+ tasks в ноте** — каждая подписана на events отдельно. **Mitigation**: один subscribe на ноту, dispatch по taskId через Map.
- **R3: race на migration** — если юзер открывает ноту, migration пишет, юзер быстро уходит → save может потерять migration. **Mitigation**: migration immediate writes через `saveEntry` synchronously в `hydrateFromEntry` ДО изменения isHydrating.
- **R4: TaskPage save loop** — onUpdate fires entity_changed, NodeView re-renders, может вызвать save. **Mitigation**: NodeView НЕ зовёт save при receive event — только update local view state.

## Plan

1. Spec (✓)
2. ark-core local emit + Rust unit test
3. TaskRef NodeView + TaskRefView.vue
4. TaskPage.vue + App.vue branch
5. listEntries extension для task_obj
6. Slash command rewire
7. Migration logic в hydrateFromEntry
8. Cleanup Pattern C
9. Guards + smoke + manual

## Out of scope justification

Rich task fields в TaskPage — нетривиальный UX дизайн (date pickers, tag selector, recurrence rule builder). Delphi уже это умеет, Eden получает кнопку «Открыть в Delphi» как escape hatch. Дальнейшее обогащение TaskPage — отдельный proof loop когда design ready.
