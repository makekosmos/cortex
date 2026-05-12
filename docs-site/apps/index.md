# Приложения

Kepler — это **четыре** desktop-приложения на Electron + **отдельный Android-стек** (две APK).

## Desktop (Electron)

| Приложение | Путь | Роль | Модель данных |
|---|---|---|---|
| [Eden](/apps/eden) | `apps/eden/ts` | заметки, дневник, typed notes | `note_obj` + кастомные типы |
| [Delphi](/apps/delphi) | `apps/delphi/ts` | задачи | `task_obj` (auto-миграция legacy todos на старте) |
| [Arrancador](/apps/arrancador) | `apps/arrancador` | игровая библиотека, playtime, бэкапы | `game_obj` + usage data |
| [Dashboard](/apps/dashboard) | `apps/dashboard` | read-only аналитика ARK | inspector, без записи |

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
- Desktop window chrome — через `DesktopChrome` / `DesktopContentSurface` из `@kepler/visuals`. Не делай свои titlebar-offset хаки.

## Состояние интеграции с ARK

| Приложение | ARK интегрирован? | Что осталось |
|---|---|---|
| Eden (desktop) | ✅ (notes как `note_obj`) | стартовая миграция legacy entries; Heart остаётся для editor/vault и one-time work |
| Delphi (desktop) | ✅ (tasks как `task_obj`) | legacy DB sidecar **удалён**; auto-migration на старте |
| Arrancador | ✅ (games как `game_obj`, usage через ARK) | завершён usage backfill |
| Dashboard | ✅ (read-only inspector) | предпочитать ARK analytics endpoints вместо raw SQL |
| Delphi (Android) + ark-service | ❌ | отдельный Room-стек; миграция на UniFFI от `ark-core` — задача на будущее |
