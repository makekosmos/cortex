# Delphi task UX overhaul

Дата: 2026-05-13
Slug: `delphi-task-ux-overhaul`

## Контекст

После сегодняшних UI правок (точка ARK, P2P в настройках, MSI билд) пользователь нашёл
ряд UX-багов и заявил список улучшений в Delphi.

## Acceptance Criteria

### AC1 — Backdrop QuickEntry не перекрывает titlebar/sidebar
При открытии QuickEntry модалка/blur лежат внутри content-области (под titlebar и справа от sidebar).
Контролы окна остаются интерактивными, sidebar не размыт.

### AC2 — Sidebar trim
Из основной (не settings) навигации убраны пункты «Календарь» и «Неделя».
Остаются: Входящие, Сегодня, Журнал, Корзина + Проекты.
Соответствующие импорты `CalendarDays`/`Kanban` из lucide-vue-next в SideBar удалены.
Чёрная иконка календаря, на которую жаловался пользователь, исчезает.

### AC3 — QuickEntry: убраны Сегодня / Вечер
В форме QuickEntry больше нет тогглов «Сегодня» и «Вечер».
Если QuickEntry открыта со страницы `/today` — `scheduledDate` автоматически = today (ISO).
Если открыта с обычной страницы — даты нет.
Дата редактируется через DateTimePicker (либо иконку календаря с popover).

### AC4 — QuickEntry: project-picker → реальный список
Кнопка «Входящие» (текущий project chip) при клике открывает список проектов
из `useTodoStore.projects` (active only) + пункт «Входящие» (projectId=null).
Выбор подсвечивается и подставляется в payload.

### AC5 — Билbable flag в QuickEntry
В форме QuickEntry есть toggle «Оплачиваемая» + опциональный input «Цена» (number).
Поля попадают в payload (`billable: boolean`, `price: number | null`).

### AC6 — TodoItem.propsJson billable
`TodoItem` тип расширен полями `billable?: boolean` и `price?: number | null`.
Сохраняются и загружаются через `@kosmos/ark` (в `task_obj.propsJson`).
Существующие задачи без этих полей загружаются как `billable=false, price=null`.

### AC7 — Inline expand on click
Левый клик по карточке задачи разворачивает её inline-форму с полями:
title, notes, scheduledDate, billable + price.
Изменения сохраняются «по месту» (через store).
Двойной клик переименования заменяется: title правится в развёрнутой панели.

### AC8 — Context menu delete
Правый клик по задаче открывает ContextMenu из `@kosmos/visuals` с пунктом «Удалить»
(destructive). Inline-кнопка «Удалить» из TodoRow удалена.

### AC9 — Сборка зелёная
`bun run --cwd apps/delphi/ts build:js` — exit 0, без TS ошибок.
typecheck (`bun x tsc --noEmit`) — exit 0.

## Out of scope

- Project-level billable (наследование цены задачи от проекта) — отдельная задача.
- Auto-распределение price по time entries — Strontium/Horologion работа.
- Календарь/неделя — удалены, не восстанавливать без отдельной задачи.
- iOS/Android — не трогаем.

## Где правим

- `apps/delphi/ts/src/components/SideBar.vue` (AC2)
- `packages/kosmos-visuals/components/QuickEntryPanel.vue` (AC1, AC3, AC4, AC5)
  — компонент шарится, но используется только Delphi-приложением (grep подтвердил).
- `apps/delphi/ts/src/components/QuickEntry.vue` (AC4, AC5, AC6 — proxy payload)
- `apps/delphi/ts/src/types/task.ts` (AC6)
- `apps/delphi/ts/src/store/todos.ts` (AC6 — newTodo defaults)
- `apps/delphi/ts/shared/task-object-migration.ts` (AC6 — preserve propsJson billable)
- `packages/kosmos-visuals/components/TodoRow.vue` (AC7, AC8 — expand + remove inline delete)
- `apps/delphi/ts/src/pages/*Page.vue` (AC7, AC8 — wire expand-state + ContextMenu)
