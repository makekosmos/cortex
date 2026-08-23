# Spec: ark-data-provider

## Task ID

`ark-data-provider`

## Original Task Statement

Создать Android-приложение **ark-data** (headless APK, без UI, без launcher-иконки) — общий ContentProvider для приложений экосистемы **Kosmos**.

Контекст:

- Экосистема называется Kosmos (бывший Kepler)
- `ark-data` — отдельный APK, пакет `com.kosmos.ark.data`
- Схема БД сейчас: **задачи** (TodoItem — для Delphi) и **заметки** (Note — для Eden)
- Delphi (пакет `com.kazui.delphi`) читает/пишет задачи через ContentProvider URI вместо собственной Room DB
- Eden будет читать/писать заметки через тот же ContentProvider
- ContentProvider authorities: `com.kosmos.ark.data`
- Схема будет расширяться со временем, но сейчас только todos + notes
- Ark-data устанавливается отдельно, нет UI, нет launcher activity

---

## Assumptions

**A1.** Новое приложение создаётся как отдельный Gradle-проект `apps/ark-data/` внутри текущего монорепо. Структура аналогична `apps/delphi/kotlin/`.

**A2.** Схема таблицы `todos` в ark-data копирует текущую Room-схему Delphi (`TodoItem`, `Project`, `Area`, `Tag`, `Heading`, `ChecklistItem`, `TodoTagCrossRef`) версия 1. Столбцы не сокращаются — ContentProvider должен быть drop-in заменой хранилища для Delphi.

**A3.** Схема таблицы `notes` для Eden — минимальная начальная версия: `id` (TEXT PK), `title` (TEXT), `body` (TEXT), `createdAt` (TEXT ISO-8601), `updatedAt` (TEXT ISO-8601), `isTrashed` (INTEGER 0/1). Расширение схемы Notes в рамках этой задачи не планируется.

**A4.** ContentProvider экспортирован с атрибутом `android:exported="true"` и защищён permission `com.kosmos.ark.data.READ_WRITE` (signature-level), чтобы только приложения Kosmos могли обращаться к нему.

**A5.** Delphi мигрирует с собственной Room DB на ContentProvider только для таблицы `todos` и связанных сущностей (`projects`, `areas`, `tags`, `headings`, `checklist_items`, `todo_tag_cross_ref`). Таблица `PendingChange` и sync-код Delphi остаются в собственной Room DB Delphi (out of scope этой задачи).

**A6.** `minSdk` для ark-data: 28 (совпадает с Delphi).

**A7.** Основная БД ark-data хранится в стандартном размещении Room: `/data/data/com.kosmos.ark.data/databases/ark_data.db`.

---

## Acceptance Criteria

### AC1 — Структура проекта

Директория `apps/ark-data/` существует и содержит корректный Gradle-проект Android:

- `app/build.gradle.kts` с `applicationId = "com.kosmos.ark.data"`, `minSdk = 28`
- Исходный код в `app/src/main/java/com/kosmos/ark/data/`
- Модуль включён в корневой `settings.gradle` монорепо (или имеет собственный)

### AC2 — Headless APK (нет UI, нет launcher-иконки)

- `AndroidManifest.xml` не содержит `<activity>` с `android.intent.category.LAUNCHER`
- Приложение не появляется в системном лаунчере после установки

### AC3 — ContentProvider зарегистрирован и защищён

- `AndroidManifest.xml` содержит `<provider>` с `android:authorities="com.kosmos.ark.data"` и `android:exported="true"`
- Permission `com.kosmos.ark.data.READ_WRITE` объявлен с `android:protectionLevel="signature"`
- Provider требует этот permission для чтения и записи

### AC4 — Room DB: таблица todos и связанные сущности

`ArkDatabase` (Room) содержит сущности, идентичные по структуре текущей схеме Delphi:

- `todos` — все столбцы `TodoItem` из Delphi: `id`, `title`, `notes`, `priority`, `scheduledDate`, `deadline`, `reminderDate`, `isToday`, `isEvening`, `isSomeday`, `isCompleted`, `completedAt`, `isCancelled`, `cancelledAt`, `isTrashed`, `sortOrder`, `headingId`, `projectId`, `areaId`, `recurrenceData`, `createdAt`
- `projects` — `id`, `title`, `notes`, `status`, `scheduledDate`, `deadline`, `sortOrder`, `colorTag`, `areaId`, `createdAt`
- `areas` — `id`, `title`, `sortOrder`, `createdAt`
- `tags` — `id`, `title`, `color`, `createdAt`
- `headings` — `id`, `title`, `sortOrder`, `projectId` (FK)
- `checklist_items` — `id`, `title`, `isCompleted`, `sortOrder`, `todoItemId` (FK CASCADE DELETE)
- `todo_tag_cross_ref` — `todoId` (FK), `tagId` (FK)

### AC5 — Room DB: таблица notes

`ArkDatabase` содержит сущность `Note` с таблицей `notes`:

- Столбцы: `id` (TEXT PK), `title` (TEXT NOT NULL), `body` (TEXT NOT NULL), `createdAt` (TEXT NOT NULL), `updatedAt` (TEXT NOT NULL), `isTrashed` (INTEGER NOT NULL default 0)

### AC6 — ContentProvider реализует CRUD для todos и связанных таблиц

`ArkDataProvider` поддерживает следующие URI и операции:

- `content://com.kosmos.ark.data/todos` — `query`, `insert`
- `content://com.kosmos.ark.data/todos/#` — `query`, `update`, `delete`
- Аналогично для `projects`, `areas`, `tags`, `headings`, `checklist_items`
- `content://com.kosmos.ark.data/todo_tag_cross_ref` — `query`, `insert`, `delete`
- `insert` возвращает URI с id новой записи; `update`/`delete` возвращают число затронутых строк

### AC7 — ContentProvider реализует CRUD для notes

`ArkDataProvider` поддерживает:

- `content://com.kosmos.ark.data/notes` — `query`, `insert`
- `content://com.kosmos.ark.data/notes/#` — `query`, `update`, `delete`

### AC8 — Delphi мигрирует с Room DB на ContentProvider для todos

В `apps/delphi/kotlin/`:

- Добавлен `ArkDataRepository` (или аналогичный класс), реализующий доступ к tasks/projects через `ContentResolver` с authority `com.kosmos.ark.data`
- Все ViewModel и UseCase, ранее использовавшие `TodoDao`/`ProjectDao` напрямую, переключены на новый репозиторий
- `DelphiDatabase` больше не включает сущности todos, projects, areas, tags, headings, checklist_items, todo_tag_cross_ref (они остаются только в ark-data)
- Приложение Delphi компилируется без ошибок

### AC9 — Delphi корректно обрабатывает отсутствие ark-data

Если ark-data не установлен или недоступен, Delphi отображает пользователю понятное сообщение об ошибке (UI-экран или toast) вместо краша с необработанным исключением.

### AC10 — Сборка ark-data

`./gradlew :apps:ark-data:app:assembleDebug` завершается без ошибок, итоговый APK содержит класс `com.kosmos.ark.data.ArkDataProvider`.

### AC11 — Сборка Delphi после миграции

`./gradlew :apps:delphi:kotlin:app:assembleDebug` завершается без ошибок после всех изменений в Delphi.

---

## Constraints

- Язык: **Kotlin**
- DI: **Hilt** (`@HiltAndroidApp`, `@Inject constructor`)
- ORM: **Room** (версия совместима с Delphi)
- `minSdk`: 28, `targetSdk`: 35
- Пакет: `com.kosmos.ark.data`
- Все идентификаторы (id) — строки UUID, lowercase (инвариант экосистемы Kosmos)
- `ContentProvider` должен быть потокобезопасен (Room обеспечивает это через DAOs)
- Новый модуль размещается в `apps/ark-data/` внутри монорепо
- Нет UI Activity, нет иконки в системном лаунчере
- Permission для доступа к provider — `signature`-level (только приложения, подписанные тем же ключом)
- `recurrenceData` сериализуется через Room TypeConverter (как в Delphi — JSON-строка)

---

## Non-Goals

- **P2P/LAN sync** — ark-data не реализует sync-протокол; синхронизация остаётся в Delphi.
- **Миграция Eden на ContentProvider** — AC7 только добавляет таблицу notes в провайдер; Eden продолжает использовать собственную БД. Миграция Eden — отдельная задача.
- **Расширение схемы notes** — теги, вложения, markdown для notes вне скоупа.
- **Таблица PendingChange** — sync-очередь Delphi остаётся в собственной Room DB Delphi.
- **Backup/export** — механизм резервного копирования БД вне скоупа.
- **ContentObserver/Flow notifications** — реактивное обновление через `notifyChange()` желательно, но не является блокером.
- **Multi-space** — один пользователь, одна БД, без изоляции по spaceId внутри ark-data.
- **Room schema migrations** — версия 1 → N миграции вне скоупа первой реализации.
- **UI настроек** — ark-data не имеет никакого пользовательского интерфейса.

---

## Verification Plan

| ID  | Шаг                                        | Команда / Проверка                                                              | Ожидаемый результат                                               |
| --- | ------------------------------------------ | ------------------------------------------------------------------------------- | ----------------------------------------------------------------- | --------------------- | ------------ |
| V1  | Структура проекта                          | `ls apps/ark-data/app/src/main/java/com/kosmos/ark/data/`                       | Директория существует, содержит `.kt` файлы                       |
| V2  | Headless: нет LAUNCHER                     | `grep -r "LAUNCHER" apps/ark-data/app/src/main/AndroidManifest.xml`             | Нет совпадений                                                    |
| V3  | Provider зарегистрирован                   | `grep "authorities" apps/ark-data/app/src/main/AndroidManifest.xml`             | `android:authorities="com.kosmos.ark.data"`                       |
| V4  | Permission signature-level                 | `grep "protectionLevel" apps/ark-data/app/src/main/AndroidManifest.xml`         | `android:protectionLevel="signature"`                             |
| V5  | Entities в ArkDatabase                     | Читать `ArkDatabase.kt`, проверить `entities = [...]`                           | Содержит все 7 todos-сущностей + `Note::class`                    |
| V6  | URI patterns в провайдере                  | Читать `ArkDataProvider.kt`, проверить `UriMatcher`                             | Все URI для todos, notes, projects, areas и т.д. зарегистрированы |
| V7  | Сборка ark-data                            | `./gradlew :apps:ark-data:app:assembleDebug`                                    | `BUILD SUCCESSFUL`                                                |
| V8  | Класс в APK                                | `unzip -p build/outputs/apk/debug/app-debug.apk classes.dex                     | strings                                                           | grep ArkDataProvider` | Класс найден |
| V9  | Сборка Delphi                              | `./gradlew :apps:delphi:kotlin:app:assembleDebug`                               | `BUILD SUCCESSFUL`                                                |
| V10 | Delphi не использует TodoDao напрямую в VM | `grep -r "todoDao\." apps/delphi/kotlin/app/src/main/java/com/kazui/delphi/ui/` | Нет совпадений                                                    |
| V11 | Обработка отсутствия ark-data              | Читать код Delphi — поиск try/catch или проверки provider                       | Обработчик присутствует                                           |
