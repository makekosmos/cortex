# Delphi

GTD-менеджер задач — часть экосистемы Kosmos. Три реализации: macOS (SwiftUI), Web (Electron + React) и Mobile (Expo + React Native).

## Платформы

| Платформа | Путь | Стек |
|-----------|------|------|
| **macOS** | `swift/` | SwiftUI + SwiftData, macOS 14+ |
| **Web/Desktop** | `ts/` | Electron + React + Vite, TypeScript |
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
