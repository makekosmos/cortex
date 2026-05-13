# Приложения

Kepler — это **пять активных** desktop-приложений на Electron, **одно зарезервированное** (Digital Cave), плюс **отдельный Android-стек** (две APK).

## Desktop (Electron)

| Приложение | Путь | Роль | Модель данных |
|---|---|---|---|
| [Eden](/apps/eden) | `apps/eden/ts` | заметки, дневник, typed notes | `note_obj` + кастомные типы |
| [Delphi](/apps/delphi) | `apps/delphi/ts` | задачи | `task_obj` (auto-миграция legacy todos на старте) |
| [Arrancador](/apps/arrancador) | `apps/arrancador` | игровая библиотека, playtime, бэкапы | `game_obj` + usage data |
| [Dashboard](/apps/dashboard) | `apps/dashboard` | read-only аналитика ARK | inspector, без записи |
| [Horologion](/apps/horologion) | `apps/horologion` | трекер времени, pomodoro + секундомер, ссылки на задачи Delphi | `time_entry_obj` + `tag_obj` (общий с Delphi) |
| [Digital Cave](/apps/digital-cave) <span class="kbadge info">TBD</span> | `apps/digital-cave` (зарезервировано) | focus-блокер (Cold Turkey Blocker аналог) | TBD |

Все desktop-приложения говорят с ARK через `@kepler/ark` и используют общие UI-компоненты из `@kepler/visuals` (Sidebar, Titlebar, DesktopChrome, и т.д.).

## Android (Kotlin)

Отдельный стек **только для Android**, изолированный от desktop ARK. Состоит из двух APK, связанных через signature-permission ContentProvider:

| APK | Путь | Роль |
|---|---|---|
| Delphi (Android) | `apps/delphi/kotlin` | UI, Compose. Package `com.kazui.delphi`. |
| [ark-service](/apps/ark-service) | `apps/ark-service` | Room SQLite + ContentProvider. Package `com.kepler.ark.data`. Хранит данные Android Delphi. |

::: warning Не путать с desktop ARK
Android-стек **сейчас не использует** `ark-core` Rust runtime — у него своя Room-база и свой ContentProvider. Sync между Android и desktop не работает. Долгосрочно планируется миграция Android Delphi на UniFFI-binding'и от `ark-core`, что позволит снести `apps/ark-service` целиком. См. [ark-service](/apps/ark-service).
:::

## Общие правила

- Renderer **никогда** не открывает SQLite напрямую — всё через preload API (`window.<app>Api`).
- Все Electron-приложения подключают `@kepler/ark` из Electron main.
- Прямые SQL writes в ARK-таблицы запрещены. См. [Граница записи](/concepts/write-boundary).
- Все тесты — на изолированных БД. См. [Изоляция тестовых БД](/concepts/test-isolation).
- **Язык UI — русский.** Все user-facing строки (placeholder, labels, кнопки, эмпти-стейты, пилюли, заголовки view) — на русском. Английский только для technical-идентификаторов (id типов объектов, имена пакетов, log message'и). Это относится ко всем приложениям без исключения.
- **Каждое приложение запоминает геометрию окна между запусками.** Сохранять `x` / `y` / `width` / `height` / `isMaximized` в `app.getPath("userData") + "/window-state.json"` на события `resize` / `move` / `maximize` / `unmaximize` / `close` (debounce 400мс на тики, final flush на close), и восстанавливать при `createWindow`. Если сохранённый файл отсутствует или битый — fallback на дефолтные дименсии приложения. Electron сам клампит bounds внутрь доступных дисплеев, если монитор отключили. Reference-имплементация: `apps/horologion/electron/main.ts` (`loadWindowState` / `saveWindowState` / `scheduleWindowStateSave`).
- **Каждое приложение имеет свой `--<app>-accent` токен.** В локальном `styles.css` приложения объявляется `--<app>-accent` (например `--horologion-accent: oklch(0.66 0.245 305)` — Apple HIG systemPurple) + `--<app>-accent-foreground`, и переопределяется общий `--accent` / `--accent-foreground` на эти значения. Все компоненты автоматически подхватят свой цвет. Цвет выбирается осмысленно (Eden — зелёный, Horologion — фиолетовый, и т.п.), желательно из официальных HIG-палитр для узнаваемости.
- **Inter Variable как fallback-шрифт.** macOS подхватит системный SF Pro раньше, но Windows и Linux должны рендерить именно Inter — мы подгружаем его через `@fontsource-variable/inter` (variable-шрифт ~30KB woff2, все weights в одном файле). Импортируется одной строкой в `src/main.ts` приложения. Дальше fallback на Segoe UI / Helvetica / Arial. Порядок прописан в `--font-sans` в `@kepler/visuals/theme/css-variables.css`.
- **Settings — отдельное Electron-окно.** Для приложений с настройками: не делать `/settings` route в основной навигации, а открывать через IPC отдельный `BrowserWindow` с hash `#/settings`. App.vue видит этот route и рендерит только `SettingsView` внутри своего `<DesktopChrome>` + `<DesktopContentSurface>` — единый стиль с main, но без основного chrome'а / контента. Reference: `apps/horologion/electron/main.ts → openSettingsWindow()` + handler `horologion:settings:open`.
- **Все desktop-приложения используют `@kepler/visuals` как единый источник UI**:
  - Chrome / safe-area — через `<DesktopChrome>` + `<DesktopContentSurface>`. Никаких ручных `--titlebar-height` хаков.
  - Цвета, радиусы, шрифты — **только** через CSS-переменные kepler-visuals (`var(--background)`, `var(--foreground)`, `var(--border)`, `var(--accent)`, `var(--radius)`, `var(--corner-shape)`, и т.д.).
  - Shared компоненты (`Sidebar`, `Titlebar`, `StatusDot`, `CommandPalette`, `TodoRow`, `GamePosterCard`, `QuickEntryPanel`, `CustomCaret`) импортируются из `@kepler/visuals`, не копируются в `apps/<name>/src/`.
  - App-specific компоненты (например `TimeEntryRow` в Horologion) живут в `apps/<name>/src/` но используют только токены и примитивы из kepler-visuals.

## Состояние интеграции с ARK

| Приложение | ARK интегрирован? | Что осталось |
|---|---|---|
| Eden (desktop) | ✅ (notes как `note_obj`) | стартовая миграция legacy entries; Heart остаётся для editor/vault и one-time work |
| Delphi (desktop) | ✅ (tasks как `task_obj`) | legacy DB sidecar **удалён**; auto-migration на старте; **TODO billing**: `propsJson.price` / `hourlyRate` на task_obj для расчёта $/час из связанных `time_entry_obj` |
| Arrancador | ✅ (games как `game_obj`, usage через ARK) | завершён usage backfill |
| Dashboard | ✅ (read-only inspector) | предпочитать ARK analytics endpoints вместо raw SQL |
| Horologion | ✅ (time_entry_obj) | tag picker UI и реальный `object_link` task↔entry — TODO (см. roadmap) |
| Digital Cave | ⏳ TBD | зарезервировано, кода нет |
| Delphi (Android) + ark-service | ❌ | отдельный Room-стек; миграция на UniFFI от `ark-core` — задача на будущее |
