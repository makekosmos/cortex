# Evidence — Delphi task UX overhaul

Дата: 2026-05-13
Spec: [spec.md](./spec.md)

## AC1 — Backdrop не перекрывает titlebar/sidebar — PASS

`apps/delphi/ts/src/App.vue:629-632` — `<QuickEntry />` теперь рендерится
**внутри** `<main class="relative">` под `<DesktopContentSurface>`, а не на root.
`packages/kosmos-visuals/components/QuickEntryPanel.vue:111` — модалка использует
`absolute inset-0` (вместо `fixed inset-0`), поэтому backdrop ограничен
content-областью. Titlebar и Sidebar остаются вне backdrop'а.

## AC2 — Sidebar trim — PASS

`apps/delphi/ts/src/components/SideBar.vue` — из импортов удалены `CalendarDays`
и `Kanban`. `primaryItems` (line 165-191) теперь содержит только
`inbox` + `today`. Календарь/Неделя как навигация исчезли.
`apps/delphi/ts/src/router/index.ts` — роуты `/calendar`, `/week`, `/upcoming`
удалены (страницы оставлены на диске, но недоступны).

## AC3 — QuickEntry: убраны Сегодня / Вечер — PASS

`packages/kosmos-visuals/components/QuickEntryPanel.vue` — кнопки `Сегодня` (Star)
и `Вечер` (Moon) удалены. `isToday/isEvening` исчезли из payload.
`apps/delphi/ts/src/components/QuickEntry.vue:21-23` — `defaultScheduledDate`
автоматически = today, если QuickEntry открыта со страницы `/today` (используется
`useRoute().path`).

## AC4 — QuickEntry: project-picker → реальный список — PASS

`packages/kosmos-visuals/components/QuickEntryPanel.vue:182-220` — chip с текстом
«Входящие» (или название выбранного проекта) теперь всегда показывается,
клик открывает dropdown со списком: пункт «Входящие» (projectId=null) +
все активные `projects` из `useTodoStore`. Выбранный пункт подсвечивается accent-цветом.

## AC5 — Billable flag в QuickEntry — PASS

`packages/kosmos-visuals/components/QuickEntryPanel.vue:142-180` — кнопка
«Оплачиваемая» (DollarSign icon, emerald-tone когда активна) переключает
`billable`. Когда активна — появляется number-input «Цена». Payload расширен
полями `billable: boolean`, `price: number | null`.

## AC6 — TodoItem.propsJson billable — PASS

`apps/delphi/ts/src/types/task.ts:307-311` — `TodoItem` теперь имеет
`billable: boolean` и `price?: number | null`.
`apps/delphi/ts/shared/task-ark.ts:347-348, 384-389` — мапперы
`todoToArkTaskObject` и `arkTaskObjectToTodo` пишут/читают эти поля
из `propsJson.billable / propsJson.price`. Старые задачи без полей
загружаются как `billable=false, price=null`.

## AC7 — Inline expand on click — PASS

`packages/kosmos-visuals/components/TodoRow.vue` — переписан. Левый клик по
row (`onRowClick`) переключает `expanded`. При expand — рендерится
inline-форма с input title, textarea notes, date input, billable toggle +
price input. Каждый commit (blur / Enter / change) эмитит `@update` с patch'ем.
Все страницы (`AllTaskPage`, `TodayPage`, `ProjectPage`) пробрасывают это в
`store.updateTodo(id, patch)`.

## AC8 — Context menu delete — PASS

`packages/kosmos-visuals/components/TodoRow.vue:127-145` — обработчик
`@contextmenu.prevent` открывает `<ContextMenu :open :x :y @close>` с
`<ContextMenuItem destructive @click="handleDelete">Удалить</ContextMenuItem>`.
Inline-кнопка «Удалить» из строки удалена. Событие `@trash` сохраняется —
теперь триггерится только из context menu.

## AC9 — Сборка зелёная — PASS

```
$ bun x tsc --noEmit
(no output, exit 0)

$ bun run build:js
✓ built in 2.08s   (renderer)
✓ built in 22ms    (main)
✓ built in 12ms    (preload)
```

См. логи в [raw/build.md](./raw/build.md).

## Out of scope / отложено

- Project-level billable (наследование цены задачи от проекта) — отдельная задача.
- Распределение price по time entries из Strontium/Horologion — отдельная задача.
- Иконка `<Calendar>` в `QuickEntryPanel` ранее имела blue-500 цвет когда дата выбрана;
  заменена на `text-(--accent)` — теперь зависит от темы, не чёрная.
- Страницы `CalendarPage.vue` / `WeekPage.vue` остались в `pages/` без роутов —
  можно удалить отдельным cleanup, но это не блокирует AC.
