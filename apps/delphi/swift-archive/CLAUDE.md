ПРОЕКТ В АРХИВЕ И НЕ РАЗВИВАЕТСЯ. НЕ ТРОГАТЬ.

# Delphi macOS (Swift)

GTD-менеджер для macOS. SwiftUI + SwiftData + Observation framework.

## Структура

```
Delphi/
├── App/
│   └── DelphiApp.swift       — @main, ModelContainer, SyncSettings, ArkPeerManager, SpaceManager
├── Space/
│   ├── SpaceManager.swift    — Ark Space: генерация кода, SHA-256, UserDefaults
│   └── SpaceSetupView.swift  — Экран настройки пространства (create/join)
├── Sync/
│   ├── SyncServer.swift      — NWListener WS-сервер (порт 21531)
│   ├── SyncClient.swift      — URLSession WS-клиент
│   ├── PeerManager.swift     — Координатор пиров, mesh discovery, multi-address
│   ├── ArkEventMapper.swift  — Маппинг ArkChange ↔ SwiftData модели
│   ├── ArkPeerProtocol.swift — Типы и протокол P2P-сообщений
│   ├── ArkHLC.swift          — Hybrid Logical Clock
│   ├── SyncSettings.swift    — UserDefaults: versionVector, spaceCode, peer addresses
│   ├── SyncSettingsView.swift — Settings UI (Space секция)
│   ├── ArkSyncClient.swift   — WebSocket к Ark relay (legacy)
│   └── ArkPairing.swift      — QR/код подключения
├── Models/                   — SwiftData модели (TodoItem, Project, Area, Tag, Heading, ChecklistItem)
├── Views/                    — ContentView, TodoRow, SmartList views
├── Resources/                — Assets, Info.plist, Entitlements
└── Commands/                 — DelphiCommands (Cmd+N, Cmd+K, …)
```

## Ark Space

### Как работает

При старте `DelphiApp.onAppear`:
```swift
if let spaceCode = spaceManager.activeSpaceCode {
    startPeerManager(spaceCode)   // P2P mode: server + client
} else {
    showSpaceSetup = true         // показать setup
}
```

### SpaceManager (`Space/SpaceManager.swift`)

- `generateSpaceCode()` → `XXXX-XXXX-XXXX` (Base32-Crockford, SecRandomCopyBytes)
- `normalizeCode(_:)` → uppercase, без тире — mesh HMAC secret
- `deriveSpaceId(from:)` → SHA-256 (CryptoKit) → первые 16 hex символов = mesh_id
- `activeSpaceCode: String?` — хранится в UserDefaults (`ark.space.activeCode`)
- `addSavedSpaceCode(_:)` — список сохранённых пространств

### SpaceSetupView (`Space/SpaceSetupView.swift`)

Fullscreen sheet с тремя состояниями: выбор → создать (показывает код) / присоединиться (ввод кода).
Callback `onSpaceJoined(code: String)` вызывается при подтверждении.

### SyncSettings

Поле `spaceCode: String?` — зеркало `ark.space.activeCode` в UserDefaults.
`isSpaceConfigured: Bool` — `!spaceCode.isEmpty`.
Version vector персистится в UserDefaults.

## Синхронизация (P2P equal peers)

macOS — равноправный пир: запускает WS-сервер (NWListener, порт 21531) И подключается как клиент к другим пирам.

### Ключевые файлы

| Файл | Роль |
|------|------|
| `Sync/SyncServer.swift` | NWListener WS-сервер (порт 21531) |
| `Sync/SyncClient.swift` | URLSession WS-клиент |
| `Sync/PeerManager.swift` | Координатор: server + client + peer list exchange |

### Version vector
Персистится в UserDefaults. НЕ регенерируется при reconnect.

### UUID normalization
Все UUID → lowercase: `todo.id.uuidString.lowercased()` в ArkEventMapper.

### SwiftData batch delete limitation
`context.delete(model: TodoItem.self)` падает при обратных связях (TodoItem/project).
Решение: fetch all → итерировать `context.delete(item)`.

### Удаление данных
Нет кнопки "Очистить данные" в UI — данные удаляются только через системные средства.

## Требования

- macOS 15.0+, Swift 6.0, Xcode 16+
- Строгий concurrency (`SWIFT_STRICT_CONCURRENCY: complete`)

## Команды

```bash
cd apps/delphi/swift
xcodegen generate    # пересоздать .xcodeproj из project.yml
open Delphi.xcodeproj
```

**Важно**: при добавлении новых Swift файлов — запустить `xcodegen generate`, иначе Xcode не подхватит.

## Конвенции

- `@Observable` + `@State` вместо `@ObservableObject`/`@Published`
- SwiftData для локального хранения
- `@MainActor` для всего UI кода
- Entitlements: sandbox + network.client + network.server
