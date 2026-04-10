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
| `data/sync/PeerManager.kt` | Координатор: UniFFI `ArkCore.startSync()` — запускает в отдельном потоке, слушает `ArkEventListener` |
| `data/sync/SyncEntityParser.kt` | Сериализация/десериализация сущностей для `broadcastChange` через Rust |
| `data/sync/PeerRecord.kt` | Типы `PeerRecord`, `LanSyncState` |
| `jniLibs/arm64-v8a/libark_core.so` | Rust `.so` — пересобирать через `cargo ndk` при изменениях `ffi.rs` |
| `com/kepler/ark/core/ark_core.kt` | UniFFI биндинги — регенерировать через `uniffi-bindgen generate --library` |

### Discovery + подключение к пирам

Вся логика (UDP beacon discovery, WS-сервер, WS-клиент, дедуп пиров) живёт в Rust `ark-core`:
- **Beacon**: `beacon.rs` UDP broadcast порт 21532, Syncthing-style дедуп по `device_id`
- **Сервер**: `sync_server.rs` tokio/tungstenite на порту 21531
- **Клиент**: `sync_client.rs` address racing — все адреса пира параллельно
- **Всё через UniFFI**: `PeerManager.kt` только вызывает `arkCore.startSync()` и обрабатывает `ArkEventListener`

**Device name**: `${Build.MANUFACTURER} ${Build.MODEL}` (`MainActivity.getDeviceName()`, `SpaceSetupViewModel.getDeviceName()`). Не захардкоживать process name.

### Dispatchers.IO — обязательно для startSync

`peerManager.start()` вызывает `arkCore.startSync()` — blocking JNI call. Запускать ТОЛЬКО в `Dispatchers.IO`:

```kotlin
// MainActivity.kt
withContext(Dispatchers.IO) {
    peerManager.start(spaceCode, deviceId, deviceName)
}

// SpaceSetupViewModel.kt
withContext(Dispatchers.IO) {
    peerManager.start(normalized, deviceId, deviceName)
}
```

Без `Dispatchers.IO`: main thread блокируется → Android ANR после 5 с → синк молча не стартует.

### Hard delete (очистка корзины)

- `TrashViewModel.emptyTrash()` вызывает `peerManager.broadcastTodoDelete(id)` для каждой удалённой задачи
- Rust-сторона транслирует `deleted: true` всем пирам

### UUID normalization
Все UUID MUST be lowercase: `.lowercase()` при сохранении в Room.

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

### Пересборка Rust артефактов

При изменениях в `packages/ark-core/rust/src/ffi.rs`:

```bash
# 1. Пересобрать .so для Android ARM64
cd packages/ark-core/rust
cargo ndk -t arm64-v8a --platform 24 \
  -o ../../../apps/delphi/kotlin/app/src/main/jniLibs \
  build --release --lib

# 2. Регенерировать UniFFI биндинги из собранного .so
cargo run --bin uniffi-bindgen generate \
  --library target/aarch64-linux-android/release/libark_core.so \
  --language kotlin \
  --out-dir /tmp/ark-kotlin-bindings/

# 3. Скопировать ark_core.kt в проект
cp /tmp/ark-kotlin-bindings/com/kepler/ark/core/ark_core.kt \
   ../../../apps/delphi/kotlin/app/src/main/java/com/kepler/ark/core/ark_core.kt
```

**ВАЖНО**: `ark_core.kt` должен быть сгенерирован из ТОГО ЖЕ `.so`, который лежит в `jniLibs/`. Рассинхрон вызывает `not enough bytes remaining in buffer` при старте.

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
