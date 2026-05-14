# Eden — заметки и дневник

::: tip Источник правды
`apps/eden/AGENTS.md`, `apps/eden/ts/AGENTS.md`, `apps/eden/README.md`
:::

Eden — основное приложение для записей: дневник, мысли, знания. Offline-first, local-first. Идея: пишешь свободно, AI-анализатор раскладывает информацию по нужным «папкам знаний» и строит персональный RAG.

## Архитектура

Три слоя:

```
┌─────────────────────────────────┐
│  Vue 3.6 Vapor UI (src/)        │  TipTap editor, компоненты, стили
├─────────────────────────────────┤
│  Electron Main Process (main/)  │  IPC, SQLite, интеграции
├─────────────────────────────────┤
│  Heart — Rust sidecar (heart/)  │  Tantivy полнотекстовый поиск
└─────────────────────────────────┘
```

- **src/** — Vue 3.6 Vapor UI: редактор (TipTap), app-specific сайдбары, настройки, typed notes; shared visuals из `@kepler/visuals`.
- **main/** — Electron main process: IPC handlers, SQLite storage (`store.ts`), Heart integration, Hevy sync, мост на ARK через `ark.ts`.
- **heart/** — Rust binary: vault filesystem manager (note types, folders, save/move/delete с hardening), stdin/stdout sidecar. **Не** search engine — search мигрирован на ARK FTS5 в `store.ts:searchEntries`.

### ARK transport (Phase 2 cutover)

`main/ark.ts` использует `@kepler/ark` с kepler-aware resolution:

- По умолчанию пытается подключиться к [Kepler host](./kepler.md) через WebSocket. Если Kepler запущен — Eden не спавнит собственный `ark-core-rpc`.
- Env-флаг `KOSMOS_KEPLER_OPTIONAL=1` включает **fallback** на self-managed sidecar (legacy режим), если Kepler недоступен. Это transitional флаг — будет убран в Phase 6.
- Eden подписывается на `sync_error`/`sync_replay` events для observability schema drift'а (см. [sync hold-and-replay](../concepts/sync.md#schema-drift-hold-and-replay-phase-2)).

## Стек

| Слой | Технология |
|---|---|
| UI | Vue 3.6 **Vapor** + TipTap + Pinia |
| Desktop | Electron 38, `electron-vite`, `vite 7` (миграция с `vite-plugin-electron`) |
| Storage | SQLite (better-sqlite3) |
| Search | Rust + Tantivy через Eden Heart sidecar |
| Lint / format | oxlint 1.57, oxfmt 0.36 |
| E2E | Playwright |
| Build | Vite + Rolldown, electron-builder |

## Структура

```
apps/eden/
├─ AGENTS.md
└─ ts/
   ├─ src/
   │  ├─ App.vue
   │  ├─ Editor.vue              # TipTap, slash, wikilinks
   │  ├─ store/                  # eden.ts (бизнес), layout.ts (UI)
   │  ├─ composables/            # useKeyboard, usePlatform, useSearch, useTitlebarSafeArea
   │  ├─ components/             # sidebar/, settings/, dialogs/, typed-notes/, spaces/
   │  └─ lib/                    # edenApi.ts, typedNotes.ts, systemTypes.ts, codeBlocks.ts
   ├─ main/
   │  ├─ main.ts                 # init, BrowserWindow, IPC handlers
   │  ├─ preload.ts
   │  ├─ store.ts                # SQLite: entries, folders, note types, trash, vault
   │  ├─ ark.ts                  # мост на @kepler/ark
   │  ├─ heart.ts                # Eden Heart sidecar
   │  ├─ hevy.ts                 # Hevy fitness API
   │  └─ hevySync.ts             # Hevy → Eden entries
   ├─ heart/                     # Rust + Tantivy
   ├─ tests/                     # Playwright E2E
   ├─ docs/                      # архитектурные решения
   ├─ electron.vite.config.ts
   ├─ electron-builder.json5
   ├─ playwright.config.ts
   └─ AGENTS.md
```

## Модель данных

Заметки и typed-notes — это ARK-объекты:

- Обычные заметки — `note_obj`.
- Custom typed notes — собственные `object_types`, регистрируемые в Eden (через страницу управления типами `src/NoteTypesScreen.tsx`).
- У `entries` (внутренний формат) есть `type_id`, `header_layout`, `header_props_json`, `schema_version`. Заголовок рендерится через `TypedHeader.vue`, тело — обычный editor body.

### Что осталось от Heart

После startup migration `note_obj` объекты — **источник правды** для shared note state. Heart остаётся для:

- editor/vault-специфики (специфичные для редактора форматы и поведение)
- one-time миграции, импорта и экспорта
- будущий специализированный Eden-only поиск, если ARK object search окажется недостаточным

Текущее runtime правило (`docs/EDEN-HEART-ARK-BOUNDARY.md`):

- `loadEntry`, `listEntries`, `listNoteTypes`, `searchEntries` — читают **только** ARK objects/object types.
- Heart entry/type reads используются startup migration, не штатными путями чтения.

## Команды

```powershell
cd apps/eden/ts
bun run dev        # сборка Rust + запуск Electron dev
bun run build      # production build (Rust release + TS + Vite)
bun run lint       # oxlint
bun run format     # oxfmt --check
bun run test:e2e   # build + Playwright
bun run package    # создать DMG/installer
```

Перед сдачей задачи **обязательно**:

```powershell
bun run build           # должен пройти без ошибок
bun run test:e2e        # должен зелёным
# минимум, если изменились только типы/локальная логика без UI:
bun run lint
bun x tsc --noEmit
```

## Ключевые решения и инварианты

- **Vapor mode**: leaf-компоненты — `<script setup vapor lang="ts">`; TipTap-компоненты — обычный VDOM. Interop включён через `vaporInterop: true` в `vite.config.ts`.
- **Pinia stores**: `useEdenStore` (бизнес-логика, save coordinator) + `useLayoutStore` (UI/сайдбары).
- **Heart остаётся**, не возвращаться к ripgrep. Расширение поиска — инкрементальный индекс в Tantivy, не новый JS-хак.
- **Storage hardening** в `main/store.ts` — не упрощать. Защита для `save`/`move`/`delete` уже есть, не ломай её.
- **tree-aware path logic**: для markdown-файлов один путь заметки, не плоские пути.
- **Desktop shell** строится через shared `DesktopChrome` и `DesktopContentSurface` из `@kepler/visuals`. **Не возвращай** ручные `--titlebar-height` / `--titlebar-left-safe-area` хаки в shell.
- **Titlebar history controls** — общий `TitlebarHistoryControls` из `@kepler/visuals`. Состояние — из локальной истории экранов/записей Eden, **не** из vue-router.
- **Shared visuals**: если компонент есть в `@kepler/visuals` — импорт через public API пакета, не deep import. Локальные `src/components/sidebar/*` — это **app-specific** контейнеры, не дубли shared UI.
- **Alias** `@/` → `src/`.
- **preload**: `vite-plugin-electron` (бывший) генерирует `preload.mjs`, не `.js`. В `main.ts` путь — `.mjs`.

## E2E нюансы

- Во время e2e окно Electron **не должно** воровать фокус.
- Не ломай background launch для тестов при изменении процесса запуска окна.
- E2E могут проверять не только UI, но и файлы на диске — не упрощай так, чтобы тест перестал ловить регрессии хранилища.

## Будущее

- `apps/eden/kotlin/` — Android-версия (планируется).
- Heart не шарится между платформами (в отличие от anytype-heart), живёт внутри `ts/`.
- Voice notes + расшифровка для дневника.
- RAG поверх Gemini-embeddings или bge-m3.
- ripgrep — точный поиск, osgrep — семантический (план).
- Книги, bookmarks (а-ля mymind), задачи (todofus).
- Все сущности — объекты ARK. Eden — удобная визуализация.

## Связанные документы

- [Модель данных ARK](/concepts/ark-objects)
- [Граница записи в ARK](/concepts/write-boundary)
- `docs/EDEN-HEART-ARK-BOUNDARY.md`
