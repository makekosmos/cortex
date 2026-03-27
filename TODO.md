# TODO — Незавершённые задачи

## Ark (packages/ark/)

### Готово
- [x] Переименование Dataverse → Ark (60 тестов проходят)
- [x] core/sync.py — SyncManager с version vectors, outbox, конфликтами (28 тестов)
- [x] server/sync_ws.py — WebSocket endpoint `/ws/sync` (6 тестов)
- [x] server/discovery.py — mDNS `_ark-sync._tcp.local.`
- [x] server/pairing.py — генерация pairing codes (`ark-XXXX`) + QR
- [x] Pairing endpoints в app.py (POST /pairing/create, POST /pairing/claim, GET /pairing/qr)
- [x] core/cli.py — CLI: ark add/task/note/list/search/stats/sync/serve/pair
- [x] Демо-БД (examples/demo.db, 500 событий)
- [x] Сервер работает на localhost:8000
- [x] CLI: `ark pair` команда (генерация кода + ASCII QR в терминале)
- [x] Фикс path traversal в `_ui_spa` (проверка `is_relative_to`)
- [x] Санитизация FTS search input (strip спецсимволов FTS5)
- [x] Field(ge=0, le=10000) на limit/offset в Pydantic моделях
- [x] Тесты для server/app.py (27 тестов, TestClient)
- [x] Тесты для pairing.py (16 тестов)
- [x] Connection pooling в Ark class (keep_alive + threading.local, 8 тестов)
- [x] E2E workflow тест (4 теста: pair→sync→broadcast)
- [x] CLAUDE.md документация

### Не завершено
- [ ] UI — решить нужен ли (Кирилл сказал "кривой, смысла нет")
- [ ] HTTP POST /events не пушит в WebSocket (только sync outbox) — рассмотреть интеграцию

## Delphi Swift (apps/delphi/swift/)

### Готово
- [x] Sync/ArkSyncClient.swift — WebSocket клиент (URLSessionWebSocketTask)
- [x] Sync/ArkEventMapper.swift — маппинг TodoItem/Project ↔ Ark events
- [x] Sync/SyncSettings.swift — настройки в UserDefaults + isPaired
- [x] Sync/SyncSettingsView.swift — pairing flow (код ark-XXXX вместо URL+key)
- [x] Sync/ArkPairing.swift — claim pairing code
- [x] Sync/ArkDiscovery.swift — mDNS Bonjour + localhost probe
- [x] Auto-connect при запуске если paired
- [x] Network entitlements (client + server)
- [x] xcodegen generate (билд проходит)

### Не завершено
(нет)

## Delphi TS (apps/delphi/ts/)

### Готово
- [x] Миграция Tauri → Electron (main.ts, preload.ts, IPC fs)
- [x] Vite 8.0.3 + vite-plugin-electron 1.0.0-beta.2
- [x] oxlint настроен (157 правил, .oxlintrc.json), Prettier оставлен (oxcfmt не на npm)
- [x] Очистка дублей (normalizeApiUrl/normalizePassphrase → src/helpers/normalize.ts)
- [x] Интеграция с Ark sync (src/services/sync/ark-client.ts)
- [x] Pairing flow в настройках (src/services/sync/pairing.ts + SettingsPage)
- [x] Auto-connect при запуске если paired
- [x] TS ошибки исправлены (LinkOff→Unlink, ES2023 lib)
- [x] Feature parity — data layer:
  - [x] Все модели (TodoItem, Project, Area, Tag, Heading, ChecklistItem, RecurrenceData)
  - [x] Smart lists / TodoFilterService (7 списков)
  - [x] Recurrence logic (daily/weekly/monthly/yearly)
  - [x] Zustand store (todos.ts) с CRUD, state transitions, checklist, recurrence
  - [x] useSmartList hook
- [x] Feature parity — UI views:
  - [x] Quick Entry (Cmd+N) — floating modal, today/evening, project picker
  - [x] Quick Open (Cmd+K) — command palette, fuzzy search
  - [x] Upcoming View — задачи по дням/неделям/месяцам
- [x] Workspace linking проверен (bun workspaces)

### Не завершено
(нет)

## Elysium (apps/elysium/)

### Готово
- [x] src/sync/ark-client.ts — WebSocket sync клиент
- [x] src/sync/mapper.ts — MealEntry/WaterEntry ↔ Ark events
- [x] src/sync/sync-store.ts — Zustand store для sync
- [x] src/sync/pairing.ts — claim pairing code
- [x] nutrition-store.ts и water-store.ts обновлены (push changes on write)
- [x] settings.tsx — pairing flow (код вместо ручного URL+key)
- [x] Auto-connect при запуске если paired
- [x] E2E тесты sync (15 тестов: outgoing, incoming, round-trip, sync-store)

### Не завершено
(нет)

## Olympia (apps/olympia/)

### Готово
- [x] CLAUDE.md документация
- [x] Ark sync интеграция (lib/sync/: ark-client, mapper, sync-store, pairing)
- [x] Sync UI в профиле (pairing flow, connect/disconnect, status)
- [x] Auto-connect при запуске если paired
- [x] Создание пользовательских упражнений (app/exercise/create.tsx)

### Не завершено
(нет)

## Общее

### Готово
- [x] `bun install` из корня монорепо
- [x] E2E workflow тест (pair → connect → create → verify)

### Не завершено
(нет — все задачи выполнены!)
