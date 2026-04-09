# Delphi Android (Kotlin)

GTD-менеджер для Android. Kotlin + Jetpack Compose + Material 3 + Room + Hilt.

## Структура

```
app/src/main/java/com/kazui/delphi/
├── data/
│   ├── db/              — Room DAO: TodoDao, ProjectDao, PendingChangeDao
│   ├── model/           — Модели данных (TodoItem, Project, Area, Tag, …)
│   ├── space/           — SpaceManager (Ark Space)
│   └── sync/            — PeerManager (UniFFI ArkCore), HLC
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
| `data/sync/LanSyncClient.kt` | OkHttp WS-клиент (`ServerInfo` хранит `deviceId` сервера) |
| `data/sync/BroadcastDiscovery.kt` | UDP beacon (port 21532) отправка + приём с дедупом по `deviceId` |
| `data/sync/PeerManager.kt` | Координатор: server + client + peer list exchange + broadcast helpers |
| `data/sync/SyncEntityParser.kt` | Сериализация/десериализация сущностей для sync (включая checklist items, tags) |

### Discovery + дедуп пиров

- **BroadcastDiscovery**: UDP broadcast (port 21532) каждые 5 с. mDNS не используется — блокируется AP isolation.
- **Дедуп beacon'ов**: `ConcurrentHashMap<deviceId, SeenPeer>`, TTL 30 с. `onPeerDiscovered` зовётся только при новом `deviceId` или изменении списка адресов. Без этого — бесконечный reconnect-спам.
- **Фильтрация анонсируемых адресов**: skip link-local (`169.254/16`, `fe80::/10`), unique-local (`fc00::/7`), loopback, виртуальные интерфейсы (`utun*`, `wg*`, `tailscale*`, `docker*`, `rmnet*`, `dummy*`, `bridge*`, `vmnet*` и т.д.). Правило применяется в `BroadcastDiscovery.sendBeacon()`.
- **Device name**: `${Build.MANUFACTURER} ${Build.MODEL}` (`MainActivity.getDeviceName()`, `SpaceSetupViewModel.getDeviceName()`). Не захардкоживать process name.
- **Single-session-per-device (SyncServer)**: `handleHello` закрывает все предыдущие authenticated-сессии с тем же `deviceId` (предварительно снимая `authenticated` флаг, чтобы не дёрнуть лишний `onPeerDisconnected`). `getConnectedPeers()` дедупит через `LinkedHashMap<deviceId, name>`.
- **Inbound + outbound = 1 запись в UI**: `PeerManager.updatePeerCounts()` мержит `syncServer.getConnectedPeers()` и `lanSyncClient.serverInfo` дедупом по `deviceId`. Если Android подключен к Electron в обе стороны — в списке одна запись.

### Синхронизируемые сущности

Все 7 типов: `todo`, `project`, `area`, `tag`, `heading`, `checklist_item`, `todo_tag_cross_ref`.

- `SyncServer.loadAllEntities()` и `LanSyncClient.buildVersionVector()` обрабатывают все типы
- `SyncEntityParser.todoToJson()` включает вложенные checklist items и tag IDs
- `SyncEntityParser.applyChecklistAndTags()` применяет вложенные данные при получении
- Trashed items включены в sync — фильтрация `isTrashed` НЕ применяется при `getAllForSync()`

### Hard delete (очистка корзины)

- `TrashViewModel.emptyTrash()` вызывает `peerManager.broadcastTodoDelete(id)` для каждой удалённой задачи
- При получении `deleted: true` — запись удаляется из БД, `versionVector.remove(entityId)`

### Version vector
Персистится в DataStore. **MUST NOT** регенерироваться с `Instant.now()` при reconnect.

### UUID normalization
Все `source_id`/`event_id` MUST be lowercase: `.lowercase()` при сохранении в Room.

### Удаление данных
Нет кнопки "Очистить данные" в UI — данные удаляются только через системные настройки (Settings → Apps → Delphi → Clear Data).

### Android Auto Backup
- `allowBackup="true"` + `fullBackupContent="@xml/backup_rules"` в обоих APK (delphi + ark-data)
- Backup rules покрывают DataStore prefs + Room DB
- Данные переживают переустановку приложения

### Space code persistence
- DataStore (`ark.space.activeCode`) — основное хранилище
- SharedPreferences fallback (`ark_space_backup`) — зеркало; авто-восстановление если DataStore вернул null
- Пространство не слетает при перезапуске

### PendingChangeDao safety
- Kotlin sync uses UniFFI `ArkCore.startSync()` via `PeerManager.kt`. Legacy `ArkSyncClient`, `ArkPeerManager`, `ArkPeerProtocol`, `ArkEventMapper` have been deleted.

## Команды

```bash
cd apps/delphi/kotlin
./gradlew assembleDebug    # сборка APK
./gradlew installDebug     # установить на подключённый девайс
./gradlew test             # unit тесты
```

## Ark Data (ContentProvider)

Данные (todos, projects, areas, tags, headings, notes) хранятся в отдельном headless APK `com.kepler.ark.data`. Delphi обращается через `ArkDataRepository` (ContentResolver).

### Ключевые файлы

| Файл | Роль |
|------|------|
| `data/repository/ArkDataRepository.kt` | ContentResolver CRUD, Flow через ContentObserver |
| `di/DatabaseModule.kt` | DatabaseProvider — PendingChange (своя Room DB) + ArkDataRepository |

### Gotchas

- **UriMatcher: `*` не `#`** — UUID содержит буквы и тире, `#` матчит только числа. Всегда `addURI(authority, "table/*", CODE)`.
- **getDatabasePath() не поддерживает подпапки** — `context.getDatabasePath("spaces/id/db")` крашит. Строй путь через `File(context.getDatabasePath("x").parentFile, "spaces/id/db")`.
- **ContentObserver на main thread** — `onChange()` вызывается на main thread. `queryAllTodos()` (IPC) нельзя вызывать напрямую — делай `ioScope.launch { query(); trySend(result) }`.
- **Не дублируй ContentProvider потоки** — каждый `getTodosFlow()` создаёт отдельный ContentObserver + IPC. Не создавай несколько ViewModel с одинаковыми потоками.

## Edge-to-Edge и клавиатура (IME)

**Рабочая конфигурация (проверена на Android 15, Nothing Phone 2a):**

```
Activity: enableEdgeToEdge() + adjustResize
```

### Архитектура insets

```
NavGraph Scaffold(contentWindowInsets = WindowInsets(0)):
  bottomBar = NavigationBar (потребляет navigationBars сама)
              СКРЫВАЕТСЯ когда WindowInsets.isImeVisible == true
  { paddingValues →
    NavHost(Modifier.padding(paddingValues))
      SmartListScaffold Scaffold(contentWindowInsets = WindowInsets(0)):
        topBar = TopAppBar (потребляет statusBars по умолчанию)
        { paddingValues →
          Column(Modifier.padding(paddingValues).imePadding()) {
            LazyColumn(Modifier.weight(1f))
            QuickAddBar()  // БЕЗ insets modifier
          }
        }
  }
```

### Правила

1. **adjustResize** в манифесте — нужен для `WindowInsets.ime`
2. **Внешний Scaffold**: `contentWindowInsets = WindowInsets(0)` — не потребляет insets сам
3. **NavigationBar** скрывается при `isImeVisible` — иначе таб-бар + imePadding = двойной отступ
4. **TopAppBar** потребляет statusBars по умолчанию — НЕ обнулять `windowInsets`
5. **imePadding()** — на Column, который содержит И контент И input bar
6. **QuickAddBar** — никаких `imePadding`, `navigationBarsPadding`, `windowInsetsPadding`
7. **Не вычитай nav из ime вручную** — с adjustResize + скрытым таб-баром imePadding() работает корректно

### Замеры (справочно)

```
ime=825px (клавиатура открыта), nav=63px, paddingBottom=0dp
```

## Конвенции

- Hilt DI везде, `@Inject constructor`
- Compose UI: Material 3, тёмная тема
- ViewModel → StateFlow → collectAsStateWithLifecycle
- DataStore<Preferences> для всех настроек (не SharedPreferences)
- minSdk 28, targetSdk 35
