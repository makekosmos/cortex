# 2026-05-17 eden-extension

## Context

Eden — последняя из 5 продуктовых апок Kosmos, оставшаяся standalone Electron-приложением (`apps/eden/ts/`). Остальные 4 (Dashboard, Horologion, Delphi, Arrancador) мигрированы как Vue extensions внутрь Kepler shell в Phase 4. Eden задерживался намеренно: TipTap + Vue 3.6 Vapor + Rust Heart sidecar + широкий preload (40+ методов) делают полную миграцию ~2-недельной работой (см. [Kepler Roadmap → Phase 6](../../docs-site/apps/kepler-roadmap.md)).

Этот proof loop — **Phase 6.0**, первая инкрементальная подзадача: получить рабочую Eden внутри Kepler shell как Vue extension с базовым note CRUD через ARK. Hevy / Heart / Code Tools / Trash / Export — отдельные follow-up задачи (см. «Out of scope»).

После этой задачи пользователь должен иметь возможность открыть Eden из Kepler launcher'а, увидеть свои заметки (note_obj), открыть заметку, отредактировать и сохранить. Standalone `apps/eden/ts/` остаётся неизменным как fallback.

## Scope

В задаче:

- Создать `extensions/eden/` с структурой Vue extension'а (как `extensions/horologion/`):
  - `manifest.json` (kind: vue, devPort 5184, размер окна как у standalone Eden)
  - `package.json` с TipTap / pinia / vue / uuid / zod / lowlight / tippy.js / @kepler/ark / @kepler/visuals
  - `vite.config.mjs` для dev server'а (HMR на порту 5184)
  - `index.html` — точка входа Vite
  - `src/main.ts` — `createApp(App).use(pinia).use(vaporInteropPlugin).mount("#root")`
- Портировать Vue UI из `apps/eden/ts/src/` в `extensions/eden/src/` (через копирование), оставив:
  - `App.vue`, `Editor.vue`, `Titlebar.vue`, `InlineCaret.ts`, `SlashCommand.ts`, `Wikilink.ts`, `WikilinkList.vue`, `SlashCommandList.vue`
  - `components/`, `composables/`, `store/`, `lib/`
  - CSS файлы (`App.css`, `Editor.css`, `Titlebar.css`, `SettingsScreen.css`, `index.css`)
- Переписать `src/lib/edenApi.ts` поверх `window.kepler.ark.request` вместо `window.api`:
  - `listEntries` → `list_objects_by_type` (type=`note_obj`)
  - `loadEntry` → `get_object`
  - `saveEntry` → `upsert_object` (type_id=`note_obj`, props_json содержит folderId/typeId/etc.)
  - `deleteEntry` → `delete_object` (soft delete через `deletedAt`)
  - `listFolders` → `list_objects_by_type` (type=`folder_obj`)
  - `createFolder` → `upsert_object` (type=`folder_obj`)
  - `moveEntryToFolder` / `moveFolderToFolder` → `upsert_object` с обновлённым props.parentId
  - `deleteFolder` → `delete_object`
  - `listNoteTypes` → `list_object_types`
  - `saveNoteType` / `deleteNoteType` → `upsert_object_type` / `delete_object_type`
  - `searchEntries` → `search_objects_fts` (либо существующий FTS5 endpoint, который уже используется в `apps/eden/ts/main/store.ts:searchEntries`)
- Удалить из `extensions/eden/` зависимости от standalone-only API:
  - Vault picker UI / setup screen — заменить на постоянно «vault initialised» (single ARK DB per user, см. spaces removal 2026-05-15)
  - Code tools (lint/format) — UI-кнопки скрыть / disabled
  - Hevy интеграция — UI скрыть / disabled, settings панель показывает «Недоступно в extension'е»
  - Trash management — UI кнопки скрыть / disabled
  - Export to markdown vault — кнопку скрыть / disabled
  - Window controls (min/max/close, zoom) — через shared `window.kepler.window.*` (close/min/max), zoom — disabled
  - Heart sidecar — не вызывается из extension'а вообще; search полностью через ARK FTS5
- Зарегистрировать команды `eden:note:create` / `eden:note:search` через `kepler.ark.request("commands.register", ...)` в `src/main.ts` (по образцу Horologion); подписать handler через `kepler.ark.subscribe("command_invoked", ...)`
- Добавить open-команду `open:eden` в `shell/electron/commands.ts` (lazy icon getter)
- Установить `extensions/eden/icon.png` (можно взять из `apps/eden/ts/public/`)
- Подключить shared preload (через kosmos kepler-extension-preload — без custom preload)
- Standalone Eden (`apps/eden/ts/`) остаётся работать **без изменений**

Не в задаче:

- Hevy fitness sync (отдельный preload capability для HTTP proxy через kepler-backend) — Phase 6.1
- Code tools lint/format (child_process capability) — Phase 6.2
- Trash management UI flows (поверх ARK soft-delete) — Phase 6.3
- Export to markdown vault (dialog + writeFile capability) — Phase 6.4
- Vault picker / multi-vault — likely permanently dropped (single ARK DB per user)
- Heart Rust sidecar relocation — не нужен; ARK FTS5 покрывает search. Сам binary остаётся внутри standalone Eden для migration/import/export utility.
- Удаление `apps/eden/ts/` directory — отложено до dogfood-валидации Phase 6 целиком
- Изменения `crates/ark-core/rust/` — никаких новых endpoint'ов не добавляем; уже есть `list_objects_by_type`, `get_object`, `upsert_object`, `delete_object`, `list_object_types`, `search_objects_fts`
- Изменения `packages/ark/` — никаких новых SDK методов; используем существующий `kepler.ark.request(operation, params)` напрямую
- E2E тесты для Eden extension'а (важны, но отдельная итерация; manual smoke в evidence достаточен)

## Acceptance Criteria

**AC1.** Файл `extensions/eden/manifest.json` существует, содержит `id: "eden"`, `kind: "vue"`, `entryHtml: "dist/index.html"`, `devPort: 5184`, `keplerApiVersion: "^1.0.0"`, осмысленные `width`/`height`/`minWidth`/`minHeight`.

**AC2.** Файл `extensions/eden/package.json` объявляет workspace-name (например `@kosmos/extension-eden`), содержит TipTap-зависимости из `apps/eden/ts/package.json`, плюс `@kepler/ark` и `@kepler/visuals` (`workspace:*`).

**AC3.** `bun install` в корне репо отрабатывает без ошибок после добавления нового workspace-пакета `extensions/eden/`.

**AC4.** `bun run --cwd shell build:extensions` собирает extension `eden` без ошибок: создаётся `extensions/eden/dist/index.html` + `dist/assets/`.

**AC5.** `bun run --cwd shell typecheck` — clean.

**AC6.** В `extensions/eden/src/lib/edenApi.ts` нет ни одного обращения к `window.api`, `window.electronAPI`, `ipcRenderer` или прямому SQL. Все вызовы идут через `window.kepler.ark.request(operation, params)`. Проверяется grep'ом: `grep -E "window\.api|window\.electronAPI|ipcRenderer|better-sqlite3" extensions/eden/src/` — пустой вывод.

**AC7.** `bun run ark:guard:writes` — зелёный (extension не пишет в ARK таблицы напрямую через SQLite).

**AC8.** В `shell/electron/commands.ts` появилась open-команда для `eden`: id `open:eden`, title `"Открыть Eden"` (или эквивалент), lazy `icon: () => extensionIconDataUri('eden')`, handler делает `openExtension('eden')`.

**AC9.** Standalone Eden остаётся collectable: `bun run --cwd apps/eden/ts typecheck` (т.е. `cd apps/eden/ts && bunx tsc --noEmit`) — clean. Полный `bun run build` для standalone Eden не запускается в AC (требует Rust toolchain для Heart), но typecheck должен проходить.

**AC10.** Smoke (manual): `bun run --cwd shell dev` запускает Kepler в dev-mode. В launcher'е (Ctrl+Shift+K) есть запись «Открыть Eden». Клик открывает extension window с Eden UI, который рендерит список существующих note_obj заметок из ARK (либо пустой state, если заметок нет). Можно открыть заметку и увидеть её TipTap content. Скриншоты/notes лежат в `.agent/tasks/2026-05-17-eden-extension/raw/smoke.md`.

## Verification commands

- `bun install` — AC3.
- `bun run --cwd shell build:extensions` — AC4.
- `bun run --cwd shell typecheck` — AC5.
- `grep -RE "window\.api|window\.electronAPI|ipcRenderer|better-sqlite3" extensions/eden/src/` (через ripgrep, должен быть empty) — AC6.
- `bun run ark:guard:writes` — AC7.
- `Test-Path extensions/eden/manifest.json` + jq/чтение полей — AC1.
- `cd apps/eden/ts; bunx tsc --noEmit` — AC9.
- Manual smoke: `bun run --cwd shell dev` → открыть Eden из launcher'а → перечислить заметки → открыть заметку → задокументировать в `raw/smoke.md` — AC10.

## Out of scope decisions

- **Hevy** не интегрируется. UI-кнопки сохраняются в коде (`SettingsScreen.vue`), но disabled с tooltip «Доступно только в standalone Eden». Это допустимо потому, что extension и standalone сосуществуют — пользователь, которому нужен Hevy, остаётся на standalone до Phase 6.1.
- **Heart sidecar**: extension не пытается его спавнить и не использует. Search 100% через ARK FTS5. Это упрощает миграцию (нет проблемы с relocation native binary в extension директорию).
- **Vault picker dropped**: после удаления spaces (2026-05-15) пользователь работает с single ARK DB. Eden setup-screen («Добро пожаловать, выберите папку») неактуален в контексте Kepler — extension сразу показывает note list. Standalone Eden остаётся со своим vault picker для legacy users.
- **note_obj и folder_obj schema**: используем уже существующую (нет миграции). Если в ходе реализации обнаруживается, что `folder_obj` не зарегистрирован как object_type — в первом `listFolders` через try/catch регистрируем lazily (тот же подход, что Delphi использует для `task_obj`).
