# Eden — Дневник и Zen Mode

Phase 6.1 (2026-05-19). Eden получил режим фокуса (zen mode) и системный
тип «Дневник» — для быстрого ежедневного письма.

## Дневник

Системный тип `system-type-journal` (см.
[`extensions/eden/src/lib/systemTypes.ts`](https://github.com/yoso-industries/kepler/blob/main/extensions/eden/src/lib/systemTypes.ts)
`SYSTEM_TYPE_JOURNAL`). Slug `journal`, icon `document-text`, color purple
`#a855f7`. Schema наследует note: одно `description` поле + `related_notes`.

Заголовок дневниковой заметки **жёстко** = ISO-дата сегодняшнего дня:
`YYYY-MM-DD` (`2026-05-19`). Deterministic, locale-independent, sortable.
Никакой свободной формы — `openTodayJournal()` всегда формирует именно этот
title.

При первом вызове `openTodayJournal()` происходит `saveNoteType(SYSTEM_TYPE_JOURNAL)`
(idempotent upsert_object_type в ARK) — без этого `saveEntry` падает на
`ensureEntryTypeAvailable` (backend не знает client-only system types).

## Команды

**Static open-команды** в `shell/electron/commands.ts` (как
`horologion:pomodoro`). Видны в launcher всегда, не зависят от того что
Eden запущен.

| id | route | действие |
|---|---|---|
| `eden:note:open-today` | `/today` | найти/создать `system-type-journal` запись с today-title, активировать zen mode |
| `eden:note:create` | `/new` | новая пустая заметка типа `note_obj` |
| `eden:open` | — | просто открыть Eden |

Route'ы обрабатываются в `extensions/eden/src/main.ts` через
`kepler.navigation.initialRoute` + `onNavigate`:

```ts
function handleRoute(route: string | null): void {
  if (route === "/today") {
    dispatchEdenCommand("eden:cmd:note:open-today", null);
  } else if (route === "/new") {
    dispatchEdenCommand("eden:cmd:note:create", null);
  }
}
```

`dispatchEdenCommand` имеет **pending queue**: если App.vue ещё не успел
зарегистрировать `onCommand` listener — события буферизуются и flush'атся
через `queueMicrotask` при первой подписке. Гарантирует доставку route'а
который приходит до Vue mount.

## Zen Mode

Скрывает sidebar / titlebar leading (back/forward/sidebar-toggle) /
titlebar center / editor header (`.editor-header { display: none }` в
`.focus-mode`). Остаётся только редактор + лёгкая acrylic подсветка.

### Управление

- **Вход**: `Ctrl+K Z` (chord, VS Code-style) — нажми `Ctrl+K`, отпусти,
  затем `Z` в течение 700 ms. Хоткей отрабатывает на любой раскладке
  (RU/EN) — `useKeyboard.ts` сравнивает `e.code === "KeyK"` /
  `"KeyZ"`, не `e.key`.
- **Выход**: двойной `Esc` в окне 600 ms, либо повторный chord `Ctrl+K Z`,
  либо клик по иконке `LoaderPinwheel` слева в titlebar.
- **Legacy**: `Ctrl+Alt+Z` оставлен как альтернатива.

### WindowControls в zen

В zen mode `WindowControls` (компонент в `@kosmos/visuals`) получает
`hide-minimize` + `hide-maximize` — остаётся только close. LoaderPinwheel
слева заменяет sidebar-toggle и history-controls.

### Char counter

Bottom-center, fixed. Рекурсивно собирает длину всех `text` node'ов из
ProseMirror doc:

```ts
function countCharsInProseMirrorDoc(json: string | null | undefined): number | null
```

Русская плюрализация: 1 → «символ», 2-4 → «символа», 5+ → «символов»,
с правильной обработкой 11-14.

### Стили текста

- `.focus-mode .ProseMirror, .focus-mode .ProseMirror p { font-weight: 500 }`
- `.focus-mode .editor-content-area { padding: 6px var(--kosmos-titlebar-inline-padding) 42vh }`
  — текст начинается на той же горизонтальной линии что иконки sidebar/titlebar
  в обычном режиме; нижний `42vh` даёт distraction-free scroll.

## Acrylic

`manifest.json` Eden:

```json
{ "windowEffect": "acrylic" }
```

`extension-host.ts` читает это поле, создаёт `BrowserWindow`:

```ts
backgroundColor: "#00000000",
backgroundMaterial: "acrylic",
```

CSS (`App.css`):

```css
.app-container { background-color: var(--bg-app); }

.app-container.focus-mode-active {
  background-color: var(--sidebar-bg-acrylic);  /* alpha 82% */
  backdrop-filter: blur(24px) saturate(140%);
}

.app-container.focus-mode-active .kosmos-titlebar,
.app-container.focus-mode-active .kosmos-desktop-chrome,
/* ...all inner surfaces... */
.app-container.focus-mode-active .ProseMirror {
  background: transparent !important;
}
```

### Почему только в zen

`backdrop-filter` создаёт **containing block** для fixed-positioned
descendants (CSS spec). Если применить его глобально на `.app-container`,
`SearchOverlay` (`<CommandPalette>` с `position: fixed inset-0`)
позиционируется относительно app-container, а не viewport — overlay
рендерится только в bounds окна, частично перекрывая sidebar/content.
В zen `Ctrl+K` не открывает поиск (только chord для exit), проблема не
воспроизводится.

### Почему single layer

Если каждый внутренний surface (titlebar / sidebar / content / editor)
имеет свой `background: var(--sidebar-bg)` с alpha 82%, после 3-4 слоёв
effective opacity ≈ `1 − 0.18⁴ ≈ 99.9%` — почти полностью непрозрачно,
backdrop не виден. Поэтому в zen все inner surfaces принудительно
transparent, `.app-container` — единственный полупрозрачный слой.

### Selector gotcha

Класс `focus-mode-active` Vue ставит на тот же элемент, что и
`.app-container` (`class="app-container focus-mode-active"`). Селектор
без пробела — `.app-container.focus-mode-active`. С пробелом
(`.focus-mode-active .app-container`) ищет вложенный `.app-container`
внутри `.focus-mode-active` — никогда не матчит.

## Dock-corner widget mode

Phase 6.1.1 (2026-05-19). Расширение zen mode: окно превращается в floating
widget, прижатый к правому верхнему углу активного display'я и закреплённый
поверх остальных окон. Удобно для «всегда под рукой» во время другой работы.

### Активация

- **В zen mode** двойной клик по title в titlebar (`.eden-titlebar-title`
  span) — toggle dock-corner.
- Повторный двойной клик — возврат к обычному zen-окну (восстанавливает
  предыдущие bounds + alwaysOnTop=false).
- Выход из zen (`Esc Esc` / chord / клик по LoaderPinwheel) автоматически
  снимает dock-corner.

### Bounds

- Ширина / высота: 360 × 560.
- Позиция: правый верхний угол active display'я, отступы `marginX = 12`,
  `marginTop = 12`.
- `alwaysOnTop: true` + `skipTaskbar: true` пока docked.

### Почему нужна no-drag зона

Electron `-webkit-app-region: drag` (используется в titlebar чтобы
перетаскивать окно курсором) перехватывает pointer events на уровне Win32
— DOM `click` / `dblclick` **не fire'ятся**. Чтобы dblclick на title
сработал, span с заголовком явно объявлен `-webkit-app-region: no-drag`
(`.eden-titlebar-title` в Eden CSS). Это «дырка» в drag-region, где
браузер видит обычные mouse-события.

### setMaximizable(false) в zen

При входе в zen Eden вызывает `window.kepler.window.setMaximizable(false)`.
На Windows это блокирует native поведение «double-click по titlebar →
maximize», которое иначе срабатывало бы поверх нашего dblclick handler'а
для dock toggle. При выходе из zen — `setMaximizable(true)`.

### IPC и preload API

`shell/electron/extension-host.ts` регистрирует:

```ts
ipcMain.handle('kepler:extension:window:toggle-dock-corner', ...)
ipcMain.handle('kepler:extension:window:is-docked', ...)
ipcMain.handle('kepler:extension:window:set-maximizable', ...)
// + push-event 'kepler:extension:window:docked-changed'
```

`extension-preload.ts` exposes:

```ts
window.kepler.window.toggleDockCorner(): Promise<boolean>  // resolves to new docked state
window.kepler.window.isDocked(): Promise<boolean>
window.kepler.window.onDockedChange(cb: (value: boolean) => void): () => void
window.kepler.window.setMaximizable(value: boolean): Promise<void>
```

### Visual marker — accent border

Когда `.app-container.eden-docked` активен, через `::before` рисуется
тонкая 2px полоса по верхней границе окна — gradient от
`var(--eden-accent-color)` (`#ff5c00`) к темнее. Визуально подчёркивает,
что окно сейчас в «закреплённом» режиме и поверх других. См.
`extensions/eden/src/index.css`.

## Eden accent color

`--eden-accent-color: #ff5c00` (Eden orange) определён в
`extensions/eden/src/index.css`. Применяется к:

- `::marker` bullet / ordered list в редакторе (`.ProseMirror ul li::marker`,
  `.ProseMirror ol li::marker`) — оранжевые маркеры списков.
- Gradient в `.app-container.eden-docked::before` — accent-полоса
  dock-corner mode.

Launcher gradient для Eden (`EDEN_GRADIENT` в `shell/electron/commands.ts`)
тоже переведён на orange: `#ff5c00 → #b33800`.

## Связанное

- [Extension host](./extension-host) — `windowEffect` field в manifest, BrowserWindow options, dock-corner / set-maximizable IPC
- [Command bus](./command-bus) — static vs dynamic команды
