# Eden — заметки и дневник

::: tip Источник правды
`extensions/eden/manifest.json`, `extensions/eden/src/`, `.agent/tasks/2026-05-17-eden-extension/` (Phase 6.0 spec), `.agent/tasks/2026-05-17-eden-cleanup-and-hardening/` (Phase 6.0.A spec).
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

- **`extensions/eden/src/`** — Vue 3.6 Vapor UI: редактор (TipTap), сайдбар, настройки, typed notes; shared visuals из `@kepler/visuals`.
- **`extensions/eden/src/lib/kepler-api-shim.ts`** — мост: эмулирует `window.api` (как у standalone Eden), внутри роутит ARK операции через `window.kepler.ark.request(...)`. Это позволяет сохранять Eden codebase без массового rewrite call-sites при миграции в extension. Прецедент — Delphi `electron-api-shim.ts`.
- **`extensions/eden/src/lib/edenApi.ts`** — публичный фасад для note CRUD / folders / search / typed-notes, импортирует функции из shim'а.
- **Heart Rust sidecar — удалён.** Search полностью через ARK FTS5 (`search_objects`). Vault filesystem manager стал не нужен — single ARK DB per user.

## Стек

| Слой | Технология |
|---|---|
| UI | Vue 3.6 **Vapor** + TipTap + Pinia |
| Транспорт | `window.kepler.ark.request` → kepler-backend WS → `ark-core-rpc` |
| Storage | ARK SQLite (через runtime, не direct access) |
| Search | ARK FTS5 (`search_objects` endpoint) |
| Build | Vite + Rolldown (per-extension, через `shell/vite.extensions.config.mjs`) |

## Структура

```
extensions/eden/
├─ manifest.json              # id, kind=vue, devPort, размер окна
├─ package.json               # workspace @kosmos/extension-eden
├─ vite.config.mjs            # dev server (HMR на :5184)
├─ index.html                 # точка входа Vite
├─ icon.png                   # иконка в launcher
├─ public/anytype/icon/…      # SVG asset'ы из исторической Eden темы
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
- **Desktop shell** строится через shared `DesktopChrome` и `DesktopContentSurface` из `@kepler/visuals`.
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
