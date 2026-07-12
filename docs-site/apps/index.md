# Приложения

Kosmos — это **Kepler host** (Electron-launcher `platform/desktop/` + backend) + first-party Vue-extension'ы внутри Kepler shell (**Eden**, Delphi, Arrancador, Akasha, Daedalus), встроенный shell-view **Dashboard** и shell-owned **Focus Session** внутри Shell command surface, **два зарезервированных имени** (Digital Cave, Kerux), плюс **отдельный Android-стек** (две APK в `mobile/`).

::: tip Live snapshot
Актуальное состояние миграций / Phase trackers — `STATUS.md` в корне репозитория. Эта страница — концептуальная карта; STATUS.md — what's in flight прямо сейчас.
:::

## Desktop host

| Приложение             | Путь                | Роль                                                                                                                                                                  |
| ---------------------- | ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Kepler](/apps/kepler) | `platform/desktop/` | Electron host + global launcher (Ctrl+Shift+K). Спавнит `kepler-backend.exe`, рутит [command bus](/concepts/command-bus), Phase 4 — extension host для остальных апок |

## Vue-extensions (внутри Kepler shell)

| Приложение                     | Путь                   | Роль                                 | Модель данных                                     |
| ------------------------------ | ---------------------- | ------------------------------------ | ------------------------------------------------- |
| [Eden](/apps/eden)             | `products/eden`        | заметки, дневник, typed notes        | `note_obj` + кастомные типы                       |
| [Delphi](/apps/delphi)         | `products/delphi`      | задачи                               | `task_obj` (auto-миграция legacy todos на старте) |
| [Arrancador](/apps/arrancador) | `incubator/arrancador` | игровая библиотека, playtime, бэкапы | `game_obj` + usage data                           |
| [Daedalus](/apps/daedalus)     | `products/daedalus`    | управление Codex-сессиями            | отдельная local SQLite, не ARK sync tables        |
| [Akasha](/apps/akasha)         | `incubator/akasha`     | EPUB-читалка                         | local JSON v1                                     |

## Встроенные shell views

| Приложение                   | Путь                                                                                                   | Роль                                                                            | Модель данных                           |
| ---------------------------- | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------- | --------------------------------------- |
| [Dashboard](/apps/dashboard) | `platform/desktop/src/views/Dashboard*.vue` + `platform/desktop/src/dashboard/`                        | встроенный ARK browser: welcome (space picker) + space view (sidebar + объекты) | read-only inspector                     |
| Focus Session                | `platform/desktop/electron/focus-session.ts` + `platform/desktop/src/components/FocusCommandPanel.vue` | фокус-таймер, цель, задача Delphi, blocklist                                    | `time_entry_obj` + `focus.active_state` |

## Зарезервированные имена

| Приложение                                                              | Путь                                  | Роль                                                                             | Модель данных                      |
| ----------------------------------------------------------------------- | ------------------------------------- | -------------------------------------------------------------------------------- | ---------------------------------- |
| [Digital Cave](/apps/digital-cave) <span class="kbadge info">TBD</span> | `apps/digital-cave` (зарезервировано) | focus-блокер (Cold Turkey Blocker аналог)                                        | TBD                                |
| [Kerux](/apps/kerux) <span class="kbadge info">TBD</span>               | `apps/kerux` (зарезервировано)        | голосовой ввод по хоткею (Superwhisper аналог; faster-whisper / Groq Whisper-v3) | TBD (опционально `voice_clip_obj`) |

Apps коннектятся к `kepler-backend` (Rust, spawn'ится Kepler host'ом) через `@kosmos/ark` WS-транспорт. Динамические команды (создание задачи Delphi, заметка Eden) регистрируются апками и доступны из Kepler launcher'а; Focus Session — shell-owned набор команд `kepler:focus-*`. См. [Command bus](/concepts/command-bus).

Все desktop-приложения говорят с ARK через `@kosmos/ark` и используют общие UI-компоненты из `@kosmos/visuals` (Sidebar, Titlebar, DesktopChrome, и т.д.).

## Android (Kotlin)

Отдельный стек **только для Android**, изолированный от desktop ARK. Состоит из двух APK, связанных через signature-permission ContentProvider:

| APK                              | Путь                           | Роль                                                                                        |
| -------------------------------- | ------------------------------ | ------------------------------------------------------------------------------------------- |
| Delphi (Android)                 | `incubator/mobile/delphi`      | UI, Compose. Package `com.kazui.delphi`.                                                    |
| [ark-service](/apps/ark-service) | `incubator/mobile/ark-service` | Room SQLite + ContentProvider. Package `com.kosmos.ark.data`. Хранит данные Android Delphi. |

::: warning Не путать с desktop ARK
Android-стек **сейчас не использует** `ark-core` Rust runtime — у него своя Room-база и свой ContentProvider. Sync между Android и desktop не работает. Долгосрочно планируется миграция Android Delphi на UniFFI-binding'и от `ark-core`, что позволит снести `incubator/mobile/ark-service` целиком. См. [ark-service](/apps/ark-service).
:::

## Общие правила

- Renderer **никогда** не открывает SQLite напрямую — всё через preload API (`window.<app>Api`).
- Все Electron-приложения подключают `@kosmos/ark` из Electron main.
- Прямые SQL writes в ARK-таблицы запрещены. См. [Граница записи](/concepts/write-boundary).
- Все тесты — на изолированных БД. См. [Изоляция тестовых БД](/concepts/test-isolation).
- **Язык UI — русский.** Все user-facing строки (placeholder, labels, кнопки, эмпти-стейты, пилюли, заголовки view) — на русском. Английский только для technical-идентификаторов (id типов объектов, имена пакетов, log message'и). Это относится ко всем приложениям без исключения.
- **Каждое приложение запоминает геометрию окна между запусками.** Для standalone Electron — сохранять `x` / `y` / `width` / `height` / `isMaximized` в `app.getPath("userData") + "/window-state.json"` на события `resize` / `move` / `maximize` / `unmaximize` / `close` (debounce 400мс на тики, final flush на close), и восстанавливать при `createWindow`. Для Vue-extensions внутри Kepler shell геометрию extension window'а сохраняет сам `platform/desktop/electron/extension-host.ts` (см. соответствующую секцию [Extension host](/concepts/extension-host)). Если сохранённый файл отсутствует или битый — fallback на дефолтные дименсии. Electron сам клампит bounds внутрь доступных дисплеев, если монитор отключили.
- **Inter Variable как fallback-шрифт.** macOS подхватит системный SF Pro раньше, но Windows и Linux должны рендерить именно Inter — мы подгружаем его через `@fontsource-variable/inter` (variable-шрифт ~30KB woff2, все weights в одном файле). Импортируется одной строкой в `src/main.ts` приложения. Дальше fallback на Segoe UI / Helvetica / Arial. Порядок прописан в `--font-sans` в `@kosmos/visuals/theme/css-variables.css`.
- **Все desktop-приложения используют `@kosmos/visuals` как единый источник UI**:
  - Chrome / safe-area — через `<DesktopChrome>` + `<DesktopContentSurface>`. Никаких ручных `--titlebar-height` хаков.
  - Цвета, радиусы, шрифты — **только** через CSS-переменные `@kosmos/visuals` (`var(--background)`, `var(--foreground)`, `var(--border)`, `var(--accent)`, `var(--radius)`, `var(--corner-shape)`, и т.д.).
  - Shared компоненты (`Sidebar`, `Titlebar`, `StatusDot`, `CommandPalette`, `TodoRow`, `GamePosterCard`, `QuickEntryPanel`, `CustomCaret`) импортируются из `@kosmos/visuals`, не копируются в extension/app source.

## Состояние интеграции с ARK

| Приложение                     | ARK интегрирован?                          | Что осталось                                                                                                                                                           |
| ------------------------------ | ------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Eden (extension)               | ✅ (notes как `note_obj`)                  | Phase 6.0 ✅: standalone `apps/eden/ts/` удалён, Heart Rust sidecar удалён, search через ARK FTS5. Дальнейшее — Phase 14 (Pinia Colada) и Phase 16 (semantic search).  |
| Delphi (desktop)               | ✅ (tasks как `task_obj`)                  | legacy DB sidecar **удалён**; auto-migration на старте; **TODO billing**: `propsJson.price` / `hourlyRate` на task_obj для расчёта $/час из связанных `time_entry_obj` |
| Arrancador                     | ✅ (games как `game_obj`, usage через ARK) | завершён usage backfill                                                                                                                                                |
| Dashboard                      | ✅ (read-only inspector)                   | предпочитать ARK analytics endpoints вместо raw SQL                                                                                                                    |
| Akasha                         | ❌                                         | v1 хранит состояние локально в `extensions-data/akasha`; ARK-backed highlights/notes — будущая фаза                                                                    |
| Digital Cave                   | ⏳ TBD                                     | зарезервировано, кода нет                                                                                                                                              |
| Kerux                          | ⏳ TBD                                     | зарезервировано, кода нет                                                                                                                                              |
| Delphi (Android) + ark-service | ❌                                         | отдельный Room-стек; миграция на UniFFI от `ark-core` — задача на будущее                                                                                              |
