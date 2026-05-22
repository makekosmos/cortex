# Eden — заметки и дневник

::: tip Источник правды
`extensions/eden/manifest.json`, `extensions/eden/src/`, `.agent/tasks/2026-05-17-eden-extension/` (Phase 6.0 spec), `.agent/tasks/2026-05-17-eden-cleanup-and-hardening/` (Phase 6.0.A spec). Phase 6.1 (2026-05-19) — журнал, zen mode, acrylic backdrop, deep-link команды.
:::

Eden — приложение для записей: дневник, мысли, знания. Offline-first, local-first. Идея: пишешь свободно, AI-анализатор раскладывает информацию по нужным «папкам знаний» и строит персональный RAG.

С Phase 6.0 (2026-05-17) Eden — **Vue extension внутри Kepler shell**. До этого был standalone Electron `Eden.exe` (`apps/eden/ts/` со своим Rust Heart sidecar для управления vault). Standalone-версия удалена в Phase 6.0.A.

## Архитектура

```
Kepler.exe (Electron host)
  └─ extension-host
      └─ extensions/eden/  (Vue bundle, TipTap editor)
            ↕
            kepler.ark.request(operation, params)  ─→  kepler-backend (WS)
                                                          ↓
                                                       ark-core-rpc (SQLite)
```

- **`extensions/eden/src/`** — Vue 3.6 Vapor UI: редактор (TipTap), сайдбар, настройки, typed notes; shared visuals из `@kosmos/visuals`.
- **`extensions/eden/src/lib/kepler-api-shim.ts`** — мост: эмулирует `window.api` (как у standalone Eden), внутри роутит ARK операции через `window.kepler.ark.request(...)`. Это позволяет сохранять Eden codebase без массового rewrite call-sites при миграции в extension. Прецедент — Delphi `electron-api-shim.ts`.
- **`extensions/eden/src/lib/edenApi.ts`** — публичный фасад для note CRUD / folders / search / typed-notes, импортирует функции из shim'а.
- **Heart Rust sidecar — удалён.** Search полностью через ARK FTS5 (`search_objects`). Vault filesystem manager стал не нужен — single ARK DB per user.

## Стек

| Слой      | Технология                                                                |
| --------- | ------------------------------------------------------------------------- |
| UI        | Vue 3.6 **Vapor** + TipTap + Pinia                                        |
| Транспорт | `window.kepler.ark.request` → kepler-backend WS → `ark-core-rpc`          |
| Storage   | ARK SQLite (через runtime, не direct access)                              |
| Search    | ARK FTS5 (`search_objects` endpoint)                                      |
| Build     | Vite + Rolldown (per-extension, через `shell/vite.extensions.config.mjs`) |

## Структура

```
extensions/eden/
├─ manifest.json              # id, kind=vue, devPort, размер окна
├─ package.json               # workspace @kosmos/extension-eden
├─ vite.config.mjs            # dev server (HMR на :5184)
├─ index.html                 # точка входа Vite
├─ icon.png                   # иконка в launcher
└─ src/
   ├─ main.ts                 # installKeplerApiShim() → createApp(App).use(pinia).mount("#root")
   ├─ App.vue                 # корневой view
   ├─ Editor.vue              # TipTap editor (lazy-load chunk)
   ├─ Titlebar.vue
   ├─ lib/
   │  ├─ kepler-api-shim.ts   # window.api эмуляция поверх kepler.ark
   │  ├─ edenApi.ts           # тонкий фасад над shim
   │  ├─ codeBlocks.ts        # язык-id mapping для TipTap CodeBlock
   │  ├─ entryTitles.ts
   │  ├─ systemTypes.ts       # системные note_obj / game_obj типы
   │  └─ typedNotes.ts        # zod schemas + header layout
   ├─ store/
   │  ├─ eden.ts              # Pinia store: entries, noteTypes, currentEntry, save coordinator
   │  └─ layout.ts            # UI: sidebar, search, zen mode
   ├─ components/             # sidebar/, settings/, typed-notes/, objects/, spaces/, dialogs/
   └─ composables/            # useKeyboard, usePlatform, useSearch, useTheme, useTitlebarSafeArea
```

## Модель данных

Заметки — это ARK-объекты:

- Обычные заметки — `note_obj`.
- **Дневниковые записи** — `system-type-journal` (`SYSTEM_TYPE_JOURNAL_ID` в `src/lib/systemTypes.ts`). Title = ISO `YYYY-MM-DD`. Один entry на день; команда «Открыть сегодняшнюю заметку» либо находит существующий, либо создаёт новый.
- Custom typed notes — собственные `object_types`, регистрируемые в Eden (через страницу управления типами).
- `entries` (внутренний формат) имеют `type_id`, `header_layout`, `header_props_json`, `schema_version`. Заголовок рендерится через `TypedHeader.vue`, тело — обычный editor body.

## Команды и сборка

```powershell
# Сборка Eden extension'а (часть Kepler shell build)
bun run --cwd shell build:extensions

# Только Eden:
bunx vite build --config shell/vite.extensions.config.mjs --mode eden

# Dev mode с HMR (поднять Vite dev server отдельно):
bun run --cwd shell dev:extensions          # все extensions, eden на :5184
bun run --cwd shell dev                     # shell + extensions вместе

# Open Eden в running Kepler shell:
# Ctrl+Shift+K → "Открыть Eden"
```

## Phase 6.1 — журнал, zen mode, acrylic (2026-05-19)

### Системный тип «Дневник»

`SYSTEM_TYPE_JOURNAL` (`system-type-journal`) — predefined note type для дневниковых записей. Slug `journal`, icon `document-text`, color `#a855f7`. Schema совпадает с обычным `note_obj` (description + related_notes), но collection_name = `"Дневник"` — это нужно чтобы dashboard / sidebar показывали отдельную группу.

`openTodayJournal()` в store (`src/store/eden.ts`):

1. Если store не hydrated — pre-fetch `listEntries()` через shim.
2. Идемпотентно persist'ит `SYSTEM_TYPE_JOURNAL` через `saveNoteType(...)` — чтобы FK-constraint не упал, если тип ещё не зарегистрирован в ARK.
3. Ищет existing journal entry с title = today ISO (`YYYY-MM-DD`).
4. Если найден — открывает. Иначе — создаёт новый entry с этим title.
5. `activeSpace = "diary"` — чтобы watch не перебивал текущий контекст.

### Open-команды

Объявлены в `extensions/eden/manifest.json::commands[]` (см. полный список в самом manifest'е, источник правды). Резолвятся `loadDeclaredCommands` в `shell/electron/extension-host.ts` — видны в launcher всегда, не зависят от того, запущен ли Eden:

| id                     | route     | Что делает                                                                                                                |
| ---------------------- | --------- | ------------------------------------------------------------------------------------------------------------------------- |
| `eden:open`            | (default) | Открыть Eden                                                                                                              |
| `eden:note:create`     | `/new`    | Создать новую заметку — Eden routing видит `/new` → `dispatchEdenCommand("eden:cmd:note:create")`                         |
| `eden:note:open-today` | `/today`  | Открыть сегодняшнюю заметку дневника — routing → `dispatchEdenCommand("eden:cmd:note:open-today")` → `openTodayJournal()` |

### Eden routing + pending dispatch queue

`extensions/eden/src/main.ts` подписан на `window.kepler.navigation`:

- `initialRoute()` — читает route, переданный shell'ом при cold start.
- `onNavigate(route → dispatch)` — при `openExtension("eden", route)` для уже-открытого окна.

Route'ы транслируются в Eden command stream:

- `/today` → `dispatchEdenCommand("eden:cmd:note:open-today")`
- `/new` → `dispatchEdenCommand("eden:cmd:note:create")`

`kepler-api-shim.ts` содержит **pending dispatch queue**: если onCommand listener ещё не зарегистрирован (например, Vue приложение ещё mounting), события буферизуются и flush'атся при первой подписке через `queueMicrotask`. Это нужно, потому что для cold start Vue приложение собирается асинхронно — без queue первое событие navigation теряется.

### Zen mode (chord shortcut + double-Esc)

Composable `src/composables/useKeyboard.ts` реализует:

- **Enter zen** — chord `Ctrl+K → Z` (VS Code-style). После `Ctrl+K` есть окно `CHORD_WINDOW_MS = 700ms` чтобы нажать `Z` (иначе `Ctrl+K` отрабатывает legacy-поиск).
- **Exit zen** — **двойной** `Escape` в пределах `DOUBLE_ESC_WINDOW_MS = 600ms`. Одиночный Esc игнорируется, чтобы пользователь случайным касанием не вылетал из focus mode.
- Хоткеи используют `e.code` (`KeyK`, `KeyZ`) вместо `e.key` — layout-agnostic: работают на RU и EN раскладках одинаково.

### Acrylic backdrop в zen mode

Eden manifest задаёт `"windowEffect": "acrylic"` — `extension-host.ts` создаёт BrowserWindow с `backgroundMaterial: "acrylic"` и `backgroundColor: "#00000000"` (см. [Extension host → Window backdrop](../concepts/extension-host#window-backdrop-acrylic-mica)).

CSS-сторона (Eden-specific):

- `.app-container` **вне** zen имеет solid `var(--bg-app)` — acrylic не виден.
- `.app-container.focus-mode-active` (включается store'ом при entry в zen): `backdrop-filter: blur(24px) saturate(140%)` + полупрозрачный фон. Все вложенные surface'ы (titlebar / sidebar / content / editor / ProseMirror) форсятся в `background: transparent !important` **только** в zen.
- Без `.focus-mode-active` все surface'ы непрозрачные — acrylic в обычном режиме отключён (для consistency с остальными extension'ами).

**Caveat — backdrop-filter и containing block.** Применить `backdrop-filter` на верхнем `body` нельзя: он создаёт containing block для `position: fixed` потомков, и `SearchOverlay` начинает позиционироваться относительно body, а не viewport'а. Поэтому backdrop-filter применяется на промежуточный `.app-container.focus-mode-active`, который сам не содержит fixed overlay'ев.

### Zen UI элементы

- **WindowControls в zen** — hide minimize/maximize через props (`hideMinimize`, `hideMaximize`), оставлен только close. Слева в titlebar — `<LoaderPinwheel>` Lucide иконка для exit'а.
- **Title в titlebar** — slot `#titlebar-center` показывает имя текущей заметки / название экрана (через App.vue).
- **Char counter** в zen — fixed bottom center, считает символы в ProseMirror doc (`countCharsInProseMirrorDoc`), с русской плюрализацией («символ / символа / символов»).
- **font-weight: 500** для текста в `.focus-mode .ProseMirror`.
- **Padding** zen content = `6px` по вертикали, `var(--kosmos-titlebar-inline-padding)` по горизонтали.

### Размеры окна

В manifest:

```json
{ "width": 1100, "height": 750, "minWidth": 450, "minHeight": 400 }
```

`minWidth` снижен с 800 до 450 (через 400), чтобы Eden корректно работал на сжатом layout'е. `minHeight` снижен с 600 до 400. Sidebar collapse fix: `.sidebar-layout:has(.kosmos-sidebar-wrapper.hidden) { background: transparent; width: 0; overflow: hidden; }` — убирает серый пустой aside, когда sidebar свёрнут.

### Surface unification

Главные content-области теперь сливаются с titlebar/sidebar в один tone:

```css
.app-container,
.app-main,
.editor-wrapper,
.editor-content-area,
.settings-screen,
.setup-container,
.setup-box,
.ProseMirror {
  background: var(--sidebar-bg);
  color: var(--color-text-primary);
}
```

Elevation surfaces (`dialog-card`, `search-overlay`, `note-type-menu`, etc.) остаются на `--color-bg-primary` — их «приподнятость» над page-уровнем читаема.

### Иконки — Anytype SVG → Lucide

Папка `public/anytype/` (легаси Anytype-тема) + dist-копии удалены. Создан `src/lib/iconResolver.ts` — mapping `name → Lucide SVG paths inline → data:URI`:

| name                                  | Lucide             |
| ------------------------------------- | ------------------ |
| `document` / `document-text` / `page` | `FileText`         |
| `game-controller`                     | `Gamepad2`         |
| `barbell`                             | `Dumbbell`         |
| `fitness`                             | `Dumbbell` (alias) |

`objectIconUri(name)` возвращает `data:image/svg+xml;...`. Все 8 точек использования в Vue/TS перебиты на этот резолвер.

### Tailwind utilities-only

В `src/index.css` добавлены:

```css
@import "tailwindcss/theme.css";
@import "tailwindcss/utilities.css";
```

Только utilities + theme (без preflight — иначе reset перебивает kosmos-tokens). Vite alias `tailwindcss → shell/node_modules/tailwindcss` живёт в `shell/vite.extensions.config.mjs` — чтобы extension резолвил тот же tailwind, что и shell, без дубль-install.

### InlineCaret отключён

`InlineCaret` TipTap extension больше **не подключается** в `Editor.vue` (widget-decoration ломал drag-selection). Файл `extensions/eden/src/InlineCaret.ts` оставлен в репо, но не импортируется ни одним call-site'ом — только комментарий в `Editor.vue` отмечает причину отключения. Кастомный курсор остаётся через `CustomCaret` из `@kosmos/visuals` (Vapor-friendly overlay над браузерным).

### Accent color

Eden использует свой accent — `--eden-accent-color: #ff5c00` (orange), задан
в `extensions/eden/src/index.css`. Применяется к:

- `::marker` bullet / ordered list в TipTap редакторе.
- Gradient border в dock-corner widget mode (`.app-container.eden-docked::before`).

Launcher gradient `EDEN_GRADIENT` (в `shell/electron/commands.ts`) синхронно
переведён на orange `#ff5c00 → #b33800`. Иконка `extensions/eden/icon.png`
обновлена (показывается в Settings → Extensions и в static open-командах
launcher'а).

### Dock-corner widget mode

В zen mode двойной клик по title в titlebar превращает Eden в floating
widget: 360×560, прижатый к правому верхнему углу, `alwaysOnTop`,
`skipTaskbar`. Повторный dblclick — обратно в обычное zen-окно. Выход из
zen автоматически снимает dock. Visual marker — тонкая 2px accent-полоса
сверху окна.

Под капотом — `window.kepler.window.toggleDockCorner()` +
`setMaximizable(false)` в zen (чтобы native «double-click → maximize» не
перехватывал dblclick handler). Полная схема — [Eden zen mode → Dock-corner
widget mode](../concepts/eden-zen-mode#dock-corner-widget-mode).

### TaskRef × ProseMirror ловушки

`TaskRef` (atom-block с native `<input>` для title) попадает в шесть неочевидных ловушек ProseMirror'а — `props.node.nodeSize` врёт для атомных нод, `StarterKit.trailingNode` навязывает trailing `<p>`, `constructor.name` ломается на минификации, PM крадёт focus через selectionchange listener (не лечится Vue `@mousedown.stop`), rubber-band + click race на классах выделения. Полный разбор и единственное правильное лечение каждой — [TaskRef × ProseMirror ловушки](../concepts/eden-taskref-pm-traps.md). Регрессии проверяются e2e-специфами `eden-task-enter.spec.ts` + `eden-selection-after-click.spec.ts` на **production-build'е** (dev-mode прячет часть багов).

### E2E coverage

`tests/e2e/eden.spec.ts → describe("eden:note:open-today")` — спека проверяет, что:

1. `commands.invoke("eden:note:open-today")` не падает.
2. После invoke создан journal entry с ISO-title.
3. `.focus-mode` класс активен на root'е (auto-enter zen после открытия журнала — UX-решение Phase 6.1).
4. Cleanup test data.

## Что было удалено (Phase 6.0.A)

- **Hevy fitness sync** — заменяется Olympia позже (отдельное приложение).
- **Code lint/format** — UI и API убраны полностью. Возможно вернётся через child_process capability в shell preload.
- **Vault picker** — single ARK DB per user (после удаления spaces 2026-05-15). Welcome screen не отображается.
- **Export to markdown** — переносится в Kepler shell как универсальный per-type export (см. [Roadmap](./kepler-roadmap.md)).
- **Heart Rust sidecar** — search через ARK FTS5, vault filesystem management не нужен.
- **Standalone `apps/eden/ts/`** — удалён, fallback больше не доступен.

## Ключевые решения и инварианты

- **Vapor mode**: leaf-компоненты — `<script setup vapor lang="ts">`; TipTap-компоненты — обычный VDOM. Interop включён через `vaporInterop: true` в `shell/vite.extensions.config.mjs`.
- **Pinia stores**: `useEdenStore` (бизнес-логика, save coordinator) + `useLayoutStore` (UI/сайдбары).
- **ARK FTS5** — единственный search engine. Не возвращаться к Tantivy/ripgrep.
- **Storage hardening через ARK** — все писи идут через `upsert_object` с runtime валидацией; никаких прямых SQL write'ов из extension TS (см. [Граница записи](../concepts/write-boundary.md)).
- **TipTap CodeBlock + lowlight** — синтакс highlight в блоках кода. Никаких runtime lint/format вызовов.
- **Desktop shell** строится через shared `DesktopChrome` и `DesktopContentSurface` из `@kosmos/visuals`.
- **Lazy Editor.vue** — `defineAsyncComponent(() => import("./Editor.vue"))` в App.vue: main bundle ~350KB, editor chunk ~1.36MB lazy-loaded при открытии заметки.

## Будущее

- **Voice notes** + расшифровка для дневника (через Kerux).
- **RAG** поверх Gemini-embeddings или bge-m3.
- **Universal per-type export** (см. [Kepler Roadmap](./kepler-roadmap.md)) — notes → markdown, runs → GPX + zip, etc.
- **Olympia** — fitness-приложение (заменит роль Hevy).

## Связанные документы

- [Модель данных ARK](../concepts/ark-objects.md)
- [Граница записи в ARK](../concepts/write-boundary.md)
- [Extension host](../concepts/extension-host.md)
- [Command bus](../concepts/command-bus.md)
