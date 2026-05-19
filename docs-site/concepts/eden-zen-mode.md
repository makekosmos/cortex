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

## Связанное

- [Extension host](./extension-host) — `windowEffect` field в manifest, BrowserWindow options
- [Command bus](./command-bus) — static vs dynamic команды
