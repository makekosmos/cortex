# Приложения

Kepler — это **четыре активных** desktop-приложения на Electron, **одно WIP** (Horologion), **одно зарезервированное** (Digital Cave), плюс **отдельный Android-стек** (две APK).

## Desktop (Electron)

| Приложение | Путь | Роль | Модель данных |
|---|---|---|---|
| [Eden](/apps/eden) | `apps/eden/ts` | заметки, дневник, typed notes | `note_obj` + кастомные типы |
| [Delphi](/apps/delphi) | `apps/delphi/ts` | задачи | `task_obj` (auto-миграция legacy todos на старте) |
| [Arrancador](/apps/arrancador) | `apps/arrancador` | игровая библиотека, playtime, бэкапы | `game_obj` + usage data |
| [Dashboard](/apps/dashboard) | `apps/dashboard` | read-only аналитика ARK | inspector, без записи |
| [Horologion](/apps/horologion) <span class="kbadge accent">WIP</span> | `apps/horologion` (code-name) | трекер времени, pomodoro, ссылки на задачи Delphi | `time_entry_obj` + `tag_obj` (общий с Delphi) |
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
| Horologion | 🟡 WIP | новые типы `time_entry_obj`, `tag_obj` (общий) — добавляются |
| Digital Cave | ⏳ TBD | зарезервировано, кода нет |
| Delphi (Android) + ark-service | ❌ | отдельный Room-стек; миграция на UniFFI от `ark-core` — задача на будущее |
