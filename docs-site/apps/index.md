# Приложения

Kosmos — это **Kepler host** (Electron-launcher `shell/` + backend) + **четыре Vue-extension'а** внутри Kepler shell (Delphi, Arrancador, Dashboard, Horologion) + **Eden** как standalone Electron (до Phase 6), **два зарезервированных имени** (Digital Cave, Kerux), плюс **отдельный Android-стек** (две APK в `mobile/`).

::: tip Live snapshot
Актуальное состояние миграций / Phase trackers — `STATUS.md` в корне репозитория. Эта страница — концептуальная карта; STATUS.md — what's in flight прямо сейчас.
:::

## Desktop host

| Приложение | Путь | Роль |
|---|---|---|
| [Kepler](/apps/kepler) | `shell/` | Electron host + global launcher (Ctrl+Shift+K). Спавнит `kepler-backend.exe`, рутит [command bus](/concepts/command-bus), Phase 4 — extension host для остальных апок |

## Vue-extensions (внутри Kepler shell)

| Приложение | Путь | Роль | Модель данных |
|---|---|---|---|
| [Delphi](/apps/delphi) | `extensions/delphi` | задачи | `task_obj` (auto-миграция legacy todos на старте) |
| [Arrancador](/apps/arrancador) | `extensions/arrancador` | игровая библиотека, playtime, бэкапы | `game_obj` + usage data |
| [Horologion](/apps/horologion) | `extensions/horologion` | трекер времени, pomodoro + секундомер, ссылки на задачи Delphi | `time_entry_obj` + `tag_obj` (общий с Delphi) |

## Встроенные shell views

| Приложение | Путь | Роль | Модель данных |
|---|---|---|---|
| [Dashboard](/apps/dashboard) | `shell/src/views/Dashboard*.vue` + `shell/src/dashboard/` | встроенный ARK browser: welcome (space picker) + space view (sidebar + объекты) | read-only inspector |

## Standalone desktop apps (Electron)

| Приложение | Путь | Роль | Модель данных |
|---|---|---|---|
| [Eden](/apps/eden) | `apps/eden/ts` | заметки, дневник, typed notes | `note_obj` + кастомные типы |
| [Digital Cave](/apps/digital-cave) <span class="kbadge info">TBD</span> | `apps/digital-cave` (зарезервировано) | focus-блокер (Cold Turkey Blocker аналог) | TBD |
| [Kerux](/apps/kerux) <span class="kbadge info">TBD</span> | `apps/kerux` (зарезервировано) | голосовой ввод по хоткею (Superwhisper аналог; faster-whisper / Groq Whisper-v3) | TBD (опционально `voice_clip_obj`) |

Apps коннектятся к `kepler-backend` (Rust, spawn'ится Kepler host'ом) через `@kepler/ark` WS-транспорт. Динамические команды (Pomodoro start, создание задачи Delphi, заметка Eden) регистрируются апками и доступны из Kepler launcher'а — см. [Command bus](/concepts/command-bus).

Все desktop-приложения говорят с ARK через `@kepler/ark` и используют общие UI-компоненты из `@kepler/visuals` (Sidebar, Titlebar, DesktopChrome, и т.д.).

## Android (Kotlin)

Отдельный стек **только для Android**, изолированный от desktop ARK. Состоит из двух APK, связанных через signature-permission ContentProvider:

| APK | Путь | Роль |
|---|---|---|
| Delphi (Android) | `mobile/delphi` | UI, Compose. Package `com.kazui.delphi`. |
| [ark-service](/apps/ark-service) | `mobile/ark-service` | Room SQLite + ContentProvider. Package `com.kosmos.ark.data`. Хранит данные Android Delphi. |

::: warning Не путать с desktop ARK
Android-стек **сейчас не использует** `ark-core` Rust runtime — у него своя Room-база и свой ContentProvider. Sync между Android и desktop не работает. Долгосрочно планируется миграция Android Delphi на UniFFI-binding'и от `ark-core`, что позволит снести `mobile/ark-service` целиком. См. [ark-service](/apps/ark-service).
:::

## Общие правила

- Renderer **никогда** не открывает SQLite напрямую — всё через preload API (`window.<app>Api`).
- Все Electron-приложения подключают `@kepler/ark` из Electron main.
- Прямые SQL writes в ARK-таблицы запрещены. См. [Граница записи](/concepts/write-boundary).
- Все тесты — на изолированных БД. См. [Изоляция тестовых БД](/concepts/test-isolation).
- **Язык UI — русский.** Все user-facing строки (placeholder, labels, кнопки, эмпти-стейты, пилюли, заголовки view) — на русском. Английский только для technical-идентификаторов (id типов объектов, имена пакетов, log message'и). Это относится ко всем приложениям без исключения.
- **Каждое приложение запоминает геометрию окна между запусками.** Для standalone Electron — сохранять `x` / `y` / `width` / `height` / `isMaximized` в `app.getPath("userData") + "/window-state.json"` на события `resize` / `move` / `maximize` / `unmaximize` / `close` (debounce 400мс на тики, final flush на close), и восстанавливать при `createWindow`. Для Vue-extensions внутри Kepler shell геометрию extension window'а сохраняет сам `shell/electron/extension-host.ts` (см. соответствующую секцию [Extension host](/concepts/extension-host)). Если сохранённый файл отсутствует или битый — fallback на дефолтные дименсии. Electron сам клампит bounds внутрь доступных дисплеев, если монитор отключили.
- **Каждое приложение имеет свой `--<app>-accent` токен.** В локальном `styles.css` приложения объявляется `--<app>-accent` (например `--horologion-accent: oklch(0.66 0.245 305)` — Apple HIG systemPurple) + `--<app>-accent-foreground`, и переопределяется общий `--accent` / `--accent-foreground` на эти значения. Все компоненты автоматически подхватят свой цвет. Цвет выбирается осмысленно (Eden — зелёный, Horologion — фиолетовый, и т.п.), желательно из официальных HIG-палитр для узнаваемости.
- **Inter Variable как fallback-шрифт.** macOS подхватит системный SF Pro раньше, но Windows и Linux должны рендерить именно Inter — мы подгружаем его через `@fontsource-variable/inter` (variable-шрифт ~30KB woff2, все weights в одном файле). Импортируется одной строкой в `src/main.ts` приложения. Дальше fallback на Segoe UI / Helvetica / Arial. Порядок прописан в `--font-sans` в `@kepler/visuals/theme/css-variables.css`.
- **Settings — отдельное окно либо route внутри extension'а.** Для standalone Electron (Eden): открывать через IPC отдельный `BrowserWindow` с hash `#/settings`. Для Vue-extensions внутри Kepler shell: route `/settings` внутри memory router'а extension'а (см. [Horologion → Topbar](./horologion.md#topbar)).
- **Все desktop-приложения используют `@kepler/visuals` как единый источник UI**:
  - Chrome / safe-area — через `<DesktopChrome>` + `<DesktopContentSurface>`. Никаких ручных `--titlebar-height` хаков.
  - Цвета, радиусы, шрифты — **только** через CSS-переменные `@kepler/visuals` (`var(--background)`, `var(--foreground)`, `var(--border)`, `var(--accent)`, `var(--radius)`, `var(--corner-shape)`, и т.д.).
  - Shared компоненты (`Sidebar`, `Titlebar`, `StatusDot`, `CommandPalette`, `TodoRow`, `GamePosterCard`, `QuickEntryPanel`, `CustomCaret`) импортируются из `@kepler/visuals`, не копируются в extension/app source.
  - App-specific компоненты (например `TimeEntryRow` в Horologion) живут в `extensions/<name>/src/` но используют только токены и примитивы из `@kepler/visuals`.

## Состояние интеграции с ARK

| Приложение | ARK интегрирован? | Что осталось |
|---|---|---|
| Eden (desktop) | ✅ (notes как `note_obj`) | стартовая миграция legacy entries; Heart остаётся для editor/vault и one-time work |
| Delphi (desktop) | ✅ (tasks как `task_obj`) | legacy DB sidecar **удалён**; auto-migration на старте; **TODO billing**: `propsJson.price` / `hourlyRate` на task_obj для расчёта $/час из связанных `time_entry_obj` |
| Arrancador | ✅ (games как `game_obj`, usage через ARK) | завершён usage backfill |
| Dashboard | ✅ (read-only inspector) | предпочитать ARK analytics endpoints вместо raw SQL |
| Horologion | ✅ (time_entry_obj) | tag picker UI и реальный `object_link` task↔entry — TODO (см. roadmap) |
| Digital Cave | ⏳ TBD | зарезервировано, кода нет |
| Kerux | ⏳ TBD | зарезервировано, кода нет |
| Delphi (Android) + ark-service | ❌ | отдельный Room-стек; миграция на UniFFI от `ark-core` — задача на будущее |
