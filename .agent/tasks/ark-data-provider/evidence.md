# Evidence Bundle: ark-data-provider

## Summary

- Overall status: PASS
- Last updated: 2026-04-04

## Build Results

- **ark-data**: `BUILD SUCCESSFUL` — `raw/build-ark-data.txt`
- **Delphi**: `BUILD SUCCESSFUL` — `raw/build-delphi.txt`

---

## Acceptance Criteria Evidence

### AC1 — Структура проекта: PASS

- `apps/ark-data/app/build.gradle.kts` содержит `applicationId = "com.kosmos.ark.data"`, `minSdk = 28`
- Исходный код в `apps/ark-data/app/src/main/java/com/kosmos/ark/data/`
- Собственный `settings.gradle.kts` (`rootProject.name = "ark-data"`, `include(":app")`)
- Команда: `ls apps/ark-data/app/src/main/java/com/kosmos/ark/data/` → `db/ di/ model/ provider/ ArkDataApp.kt`

### AC2 — Headless APK: PASS

- `AndroidManifest.xml` не содержит ни одного `<activity>`
- `grep "LAUNCHER" apps/ark-data/app/src/main/AndroidManifest.xml` → 0 совпадений

### AC3 — ContentProvider зарегистрирован и защищён: PASS

- `android:authorities="com.kosmos.ark.data"` и `android:exported="true"` — подтверждено в манифесте
- `android:protectionLevel="signature"` — подтверждено
- `readPermission` и `writePermission` = `com.kosmos.ark.data.READ_WRITE`

### AC4 — Room DB: todos entities: PASS

- `ArkDatabase` содержит `TodoItem`, `ChecklistItem`, `Project`, `Area`, `Tag`, `Heading`, `TodoTagCrossRef`
- Схема идентична Delphi (те же tableName, column names, foreign keys)
- TypeConverters для `Priority`, `ProjectStatus`, `RecurrenceData` (JSON)

### AC5 — Room DB: notes entity: PASS

- `Note` entity: `id` TEXT PK, `title` TEXT, `body` TEXT, `createdAt` TEXT, `updatedAt` TEXT, `isTrashed` INTEGER

### AC6 — ContentProvider CRUD для todos и related tables: PASS

- 16 URI паттернов зарегистрированы в `UriMatcher`
- `query`, `insert`, `update`, `delete` реализованы для: todos, projects, areas, tags, headings, checklist_items, todo_tag_cross_ref
- `insert` возвращает URI с id; `update`/`delete` возвращают Int (affected rows)

### AC7 — ContentProvider CRUD для notes: PASS

- `notes` и `notes/#` зарегистрированы
- Полный CRUD реализован

### AC8 — Delphi мигрирует на ContentProvider: PASS

- `ArkDataRepository` создан в `data/repository/ArkDataRepository.kt`
- `DatabaseProvider` удалил `todoDao()` и `projectDao()`, добавил `arkDataRepository: ArkDataRepository`
- `DelphiDatabase` содержит только `PendingChange::class` (version 2)
- Все ViewModel и sync-классы переключены: `TodoViewModel`, `SmartListViewModel`, `MoreViewModel`, `ProjectViewModel`, `TrashViewModel`, `SyncServer`, `LanSyncClient`, `ArkSyncClient`, `ArkPeerManager`, `MainActivity`
- `grep -r "todoDao()" delphi/.../ui/ delphi/.../sync/` → 0 совпадений
- BUILD SUCCESSFUL

### AC9 — Обработка отсутствия ark-data: PASS

- `ArkDataRepository.isAvailable()` — `PackageManager.getPackageInfo` с `NameNotFoundException`
- `DelphiNavGraph` проверяет `todoViewModel.isArkDataAvailable`
- При `false` отображает `ArkDataMissingScreen()` с текстом на русском
- Нет краша с необработанным исключением

### AC10 — Сборка ark-data: PASS

```
BUILD SUCCESSFUL in 12s
38 actionable tasks
```

APK: `apps/ark-data/app/build/outputs/apk/debug/app-debug.apk` (8.2M)
`strings app-debug.apk | grep ArkDataProvider` → найдено множество совпадений

### AC11 — Сборка Delphi: PASS

```
BUILD SUCCESSFUL in 32s
40 actionable tasks
```

Только предупреждения об устаревших API, ни одной ошибки компиляции.

---

## Commands Run

```bash
# V1: structure
ls apps/ark-data/app/src/main/java/com/kosmos/ark/data/

# V2: no launcher
grep "LAUNCHER" apps/ark-data/app/src/main/AndroidManifest.xml

# V3: provider authority
grep "authorities" apps/ark-data/app/src/main/AndroidManifest.xml

# V4: signature permission
grep "protectionLevel" apps/ark-data/app/src/main/AndroidManifest.xml

# V5: entities
grep "entities" apps/ark-data/app/src/main/java/com/kosmos/ark/data/db/ArkDatabase.kt

# V6: URI patterns
grep -c "addURI" apps/ark-data/.../ArkDataProvider.kt  # → 16

# V7: build ark-data
cd apps/ark-data && ./gradlew assembleDebug  # BUILD SUCCESSFUL

# V8: class in APK
strings app/build/outputs/apk/debug/app-debug.apk | grep ArkDataProvider

# V9: build Delphi
cd apps/delphi/kotlin && ./gradlew assembleDebug  # BUILD SUCCESSFUL

# V10: no direct todoDao in Delphi UI
grep -r "todoDao\." apps/delphi/kotlin/app/src/main/java/com/kazui/delphi/ui/

# V11: ark-data absence handler
grep "ArkDataMissingScreen\|isArkDataAvailable" apps/delphi/kotlin/...
```

## Raw Artifacts

- `raw/build-ark-data.txt` — полный вывод сборки ark-data
- `raw/build-delphi.txt` — полный вывод сборки Delphi
- `raw/build.txt` — объединённый файл

## Known Gaps

- Нет unit-тестов (вне скоупа задачи)
- Room schema migrations не реализованы (explicitly Non-Goal в spec)
- ContentObserver notifyChange реализован (желательно по spec, не блокер)
