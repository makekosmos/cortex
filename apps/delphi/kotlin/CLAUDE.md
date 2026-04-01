# Delphi Android (Kotlin)

GTD-менеджер для Android. Kotlin + Jetpack Compose + Material 3 + Room + Hilt.

## Структура

```
app/src/main/java/com/kazui/delphi/
├── data/
│   ├── db/              — Room DAO: TodoDao, ProjectDao, PendingChangeDao
│   ├── model/           — Модели данных (TodoItem, Project, Area, Tag, …)
│   ├── space/           — SpaceManager (Ark Space)
│   └── sync/            — SyncServer, LanSyncClient, PeerManager, ArkEventMapper, HLC
├── di/                  — Hilt модули (AppModule, DatabaseModule)
├── domain/
│   └── filter/          — TodoFilterService
├── ui/
│   ├── navigation/      — NavGraph (главный NavHost + проверка Space)
│   ├── components/      — ConnectionIndicator, TodoRow, QuickAddBar, …
│   └── screens/
│       ├── space/       — SpaceSetupScreen, SpaceSetupViewModel
│       ├── settings/    — SettingsScreen, SettingsViewModel (+ Space секция)
│       ├── today/       — TodayScreen
│       ├── inbox/       — InboxScreen
│       ├── upcoming/    — UpcomingScreen
│       ├── logbook/     — LogbookScreen
│       ├── trash/       — TrashScreen
│       └── project/     — ProjectScreen
├── DelphiApp.kt         — @HiltAndroidApp
└── MainActivity.kt      — Edge-to-edge, 120Hz, mDNS discovery
```

## Ark Space

### Как работает

При старте `DelphiNavGraph` читает `SpaceManager.activeSpaceCode` из DataStore.
- **Нет кода** → показывается `SpaceSetupScreen` (create/join)
- **Код есть** → обычный nav с TabBar

### SpaceManager (`data/space/SpaceManager.kt`)

- `generateSpaceCode()` → `XXXX-XXXX-XXXX` (Base32-Crockford, SecureRandom)
- `normalizeCode(code)` → uppercase, без тире — используется как mesh secret
- `deriveSpaceId(code)` → SHA-256(normalized)[:16 hex] — mesh_id для mDNS
- `setActiveSpaceCode(code)` / `clearActiveSpaceCode()` — DataStore persistence
- `isValidCode(code)` — проверка 12 символов алфавита

Ключ DataStore: `ark.space.activeCode`

### NavGraph интеграция

`DelphiNavGraph` использует `SpaceSetupViewModel` (hiltViewModel):
```kotlin
if (!isInitialized) return  // ждём DataStore
if (activeSpaceCode == null) {
    SpaceSetupScreen(...)   // нет пространства
    return
}
// обычный NavHost...
```

### SpaceSetupScreen

Три режима: CHOOSE → CREATE (генерирует код) / JOIN (вводит код).
После `activateSpace(code)` DataStore обновляется → NavGraph реагирует → показывает основной UI.

### Settings (SettingsScreen)

Секция "ARK SPACE (P2P)":
- Показывает активный код пространства
- Кнопка "Покинуть пространство" (`leaveSpace()` в SettingsViewModel)

## Синхронизация (P2P equal peers)

Android — равноправный пир: запускает WS-сервер (Ktor CIO, порт 21531) И подключается как клиент к другим пирам.

### Ключевые файлы

| Файл | Роль |
|------|------|
| `data/sync/SyncServer.kt` | Ktor CIO embedded WS-сервер (порт 21531, fallback 21531-21541) |
| `data/sync/LanSyncClient.kt` | OkHttp WS-клиент |
| `data/sync/PeerManager.kt` | Координатор: server + client + peer list exchange |

### Version vector
Персистится в DataStore. **MUST NOT** регенерироваться с `Instant.now()` при reconnect.

### UUID normalization
Все `source_id`/`event_id` MUST be lowercase: `.lowercase()` при получении из ArkEventMapper.

### Удаление данных
Нет кнопки "Очистить данные" в UI — данные удаляются только через системные настройки (Settings → Apps → Delphi → Clear Data).

## Команды

```bash
cd apps/delphi/kotlin
./gradlew assembleDebug    # сборка APK
./gradlew installDebug     # установить на подключённый девайс
./gradlew test             # unit тесты
```

## Конвенции

- Hilt DI везде, `@Inject constructor`
- Compose UI: Material 3, тёмная тема
- ViewModel → StateFlow → collectAsStateWithLifecycle
- DataStore<Preferences> для всех настроек (не SharedPreferences)
- minSdk 28, targetSdk 35
