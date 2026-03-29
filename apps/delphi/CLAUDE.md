# Delphi

GTD-менеджер задач — часть экосистемы Kosmos. Три реализации: macOS (SwiftUI), Web (Electron + Vue) и Mobile (Expo + React Native).

## Платформы

| Платформа | Путь | Стек |
|-----------|------|------|
| **macOS** | `swift/` | SwiftUI + SwiftData, macOS 14+ |
| **Web/Desktop** | `ts/` | Electron 41 + Vue 3 (Composition API, `<script setup>`) + Vite 8 + Pinia + reka-ui + Tailwind CSS 4 |
| **Mobile** | `mobile/` | Expo + React Native, TypeScript |

Все версии синхронизируют данные с Ark через WebSocket (`/ws/sync`).

## Навигация по умолчанию

- **Мобильные устройства** (iOS/Android): главный экран при открытии — **Сегодня**
- **Десктоп** (macOS/Electron): главный экран при открытии — **Входящие** (Inbox)

## Модели данных

### TodoItem (задача)
```
id            UUID
title         String
notes         String?
priority      none | low | medium | high
scheduledDate Date?          — когда запланирована
deadline      Date?          — дедлайн
reminderDate  Date?          — напоминание
isToday       Bool           — помечена как "сегодня"
isEvening     Bool           — помечена как "вечер"
isSomeday     Bool           — отложена на "потом"
isCompleted   Bool
completedAt   Date?
isCancelled   Bool
cancelledAt   Date?
isTrashed     Bool
sortOrder     Int            — порядок в списке
headingID     UUID?          — заголовок внутри проекта
createdAt     Date

Связи:
  → Project?                 — проект (опционально)
  → Area?                    — область (опционально)
  → [Tag]                    — теги (many-to-many)
  → [ChecklistItem]          — чеклист (cascade delete)
  → RecurrenceData?          — повторение
```

### Project (проект)
```
id            UUID
title         String
notes         String?
status        active | someday | completed
scheduledDate Date?
deadline      Date?
sortOrder     Int
colorTag      String?
createdAt     Date

Связи:
  → [TodoItem]               — задачи
  → [Heading]                — заголовки-секции (cascade delete)
  → Area?                    — область
```

### Area (область)
```
id            UUID
title         String
sortOrder     Int
createdAt     Date

Связи:
  → [Project]                — проекты
  → [TodoItem]               — задачи вне проектов
```

### Tag
```
id            UUID
title         String
color         String?
createdAt     Date

Связи:
  → [TodoItem]               — many-to-many
```

### Heading (заголовок внутри проекта)
```
id            UUID
title         String
sortOrder     Int

Связи:
  → Project                  — родительский проект
```

### ChecklistItem (элемент чеклиста)
```
id            UUID
title         String
isCompleted   Bool
sortOrder     Int

Связи:
  → TodoItem                 — родительская задача
```

### RecurrenceData (повторение)
```
frequency       daily | weekly | monthly | yearly
interval        Int (каждые N единиц)
recurrenceType  fixed | afterCompletion
daysOfWeek      [Int]?       — для weekly
endDate         Date?
```

## Smart Lists (умные списки)

| Список | Фильтр |
|--------|--------|
| **Входящие** | Нет проекта, не today/evening/someday, не завершена, не в корзине |
| **Сегодня** | isToday=true ИЛИ scheduledDate=сегодня, не завершена |
| **Планы** | scheduledDate в будущем, не today/someday, не завершена |
| **Когда угодно** | Не someday, не завершена, не в корзине, нет scheduledDate в будущем |
| **Потом** | isSomeday=true, не завершена |
| **Журнал** | isCompleted=true ИЛИ isCancelled=true |
| **Корзина** | isTrashed=true |

## Клавиши (macOS)

| Действие | Сочетание |
|----------|-----------|
| Новая задача | Cmd+N |
| Новый проект | Cmd+Option+N |
| Новый заголовок | Cmd+Shift+N |
| Завершить | Cmd+K |
| Отменить | Cmd+Option+K |
| Дублировать | Cmd+D |
| Переместить | Cmd+Shift+M |
| Сегодня | Cmd+T |
| Вечер | Cmd+E |
| Когда угодно | Cmd+R |
| Потом | Cmd+O |
| Дедлайн | Cmd+Shift+D |
| Теги | Cmd+Shift+T |
| Поиск | Cmd+F |
| Сайдбар | Cmd+/ |
| Smart List 1-6 | Cmd+1..6 |

## Архитектура TS-версии (`ts/`)

### Структура

```
ts/
├── electron/              — Electron main process
│   ├── main.ts            — точка входа, IPC-хендлеры
│   ├── peer-discovery.ts  — mDNS (bonjour-service), _ark-peer._tcp
│   ├── peer-manager.ts    — P2P mesh: outbound WS, HMAC auth, broadcast
│   ├── peer-server.ts     — inbound WS для входящих peer-подключений
│   └── peer-protocol.ts   — типы и протокол P2P-сообщений
├── src/
│   ├── App.vue            — корневой layout, connection bootstrap, P2P bridge
│   ├── main.ts            — createApp, router, Pinia
│   ├── components/        — UI-компоненты (SideBar, QuickEntry, QuickOpen, TodoRow, …)
│   ├── pages/             — route views (TodayPage, AllTaskPage, ProjectPage, …)
│   ├── composables/       — useSmartList, useQuickEntry, useTheme
│   ├── store/
│   │   ├── todos.ts       — Pinia store: задачи, проекты, CRUD, Ark sync helpers
│   │   └── tasks.ts       — вспомогательные утилиты для задач
│   ├── services/
│   │   ├── sync/          — ark-client (WS), hlc (Hybrid Logical Clock), pairing, peer-bridge
│   │   ├── api/           — HTTP helpers
│   │   ├── filters/       — smart list фильтры
│   │   ├── gemini/        — голосовой ввод (Gemini Live API)
│   │   ├── recurrence/    — повторяющиеся задачи
│   │   ├── runtime/       — runtime utilities
│   │   └── storage/       — localStorage wrappers
│   ├── router/            — vue-router конфиг
│   └── types/             — TypeScript типы (Task, Project, Priority, …)
└── vite.config.ts
```

### UI-библиотека

reka-ui (headless Vue 3 components): Tooltip, Dialog и т.д. Стили — Tailwind CSS 4 с CSS-переменными (`--background`, `--foreground`, `--border`, `--popover`, `--muted-foreground`).

### Состояние подключения (App.vue)

Индикатор-точка в правом верхнем углу:
- **Зелёный** (`online`) — WebSocket с Ark активен, realtime sync работает
- **Жёлтый пульсирующий** (`syncing`) — устанавливается соединение
- **Красный** (`offline`) — соединение потеряно или не настроено

При наведении — тултип (reka-ui Tooltip) с описанием текущего состояния.

### Синхронизация (renderer)

`ArkSyncClient` (`services/sync/ark-client.ts`):
- WebSocket с version vectors (localStorage)
- Outbox для offline-изменений
- Heartbeat ping/pong
- Reconnect с exponential backoff (max 30s)
- `onStatus(cb)` / `onChange(cb)` для подписки

### P2P mesh (Electron main process)

- mDNS discovery → peer-manager координирует outbound WS
- HMAC auth, hop_path для предотвращения петель
- IPC bridge `peer:change` → renderer обрабатывает как Ark changes

## Сервисы

### TodoFilterService
Централизованная фильтрация по smart lists. Кеширует счётчики.

### LemmaSearchService
Морфологический поиск через NLTagger. Русские склонения, английские формы.

### NaturalDateParser
Парсинг дат из текста:
- "сегодня", "завтра", "послезавтра"
- "3d", "2w", "3mo", "1y"
- "in 5 days", "next monday"
- Русские дни недели, относительные фразы

## Синхронизация с Ark

Оба клиента (Swift и TS) подключаются к Ark через WebSocket:
```
ws://ark-server/ws/sync?key=API_KEY
```

Маппинг в Ark-события:
- TodoItem → `event_type: "task"`, `category: "productivity"`, `source: "delphi"`
- Project → `event_type: "project"`, `category: "productivity"`
- Area → `event_type: "area"`, `category: "productivity"`
- Tag → `event_type: "tag"`, `category: "productivity"`

Дедупликация: `source_id` = UUID объекта.

## Голосовой ввод (Web)

Google Gemini 2.5 Live API:
- Микрофон → PCM 16kHz → Gemini
- Function calling: create_task, update_last_task, delete_last_task
- Язык: русский (приоритет)

## Conventions

- Язык UI: русский
- Package manager: bun (workspace)
- Шрифт: Zed Mono Extended (web), системный (macOS)
- Тема: тёмная по умолчанию, поддержка светлой
- Path alias: `@/` → `src/`
- Vue: Composition API + `<script setup lang="ts">`, без Options API
- State: Pinia stores (`defineStore`), `shallowRef` для примитивов
- UI-компоненты: reka-ui (headless) + Tailwind CSS 4
- Линтер: oxlint, форматтер: prettier
- Тесты: vitest (unit), playwright (e2e)
