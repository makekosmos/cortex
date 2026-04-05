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
