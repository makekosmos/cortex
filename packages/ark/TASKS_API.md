# Ark — Task Sync API

Описание протокола синхронизации задач между клиентами (Android, macOS, Web) и Ark.

## Обзор

Все операции с задачами идут через WebSocket `/ws/sync`. REST-эндпоинтов для задач нет.
Ark — это event store: каждая задача — это событие с `event_type = "task"`.

```
Клиент ──WS──► Ark ──broadcast──► Другие клиенты
```

---

## Подключение

```
ws://ark-host:8000/ws/sync?key=API_KEY
```

После установки WebSocket клиент сразу отправляет `sync_start`.

---

## Структура задачи (Task)

Задача передаётся внутри поля `data.data` сообщения `change`.

| Поле | Тип | Описание |
|------|-----|----------|
| `id` | string (UUID) | Уникальный ID задачи |
| `title` | string | Заголовок |
| `notes` | string? | Заметки/описание |
| `priority` | int | `0` = нет, `1` = низкий, `2` = средний, `3` = высокий |
| `scheduledDate` | string? | ISO 8601 — когда запланирована |
| `deadline` | string? | ISO 8601 — дедлайн |
| `reminderDate` | string? | ISO 8601 — напоминание |
| `isToday` | bool | Помечена «на сегодня» |
| `isEvening` | bool | Помечена «на вечер» |
| `isSomeday` | bool | Отложена «на потом» |
| `isCompleted` | bool | Завершена |
| `completedAt` | string? | ISO 8601 — когда завершена |
| `isCancelled` | bool | Отменена |
| `cancelledAt` | string? | ISO 8601 — когда отменена |
| `isTrashed` | bool | В корзине |
| `sortOrder` | int | Порядок в списке (меньше = выше) |
| `headingId` | string? | ID заголовка внутри проекта |
| `projectId` | string? | ID проекта |
| `areaId` | string? | ID области |
| `createdAt` | string | ISO 8601 — дата создания |

---

## Протокол WebSocket

### 1. `sync_start` — клиент → сервер

Первое сообщение после подключения. Передаёт version vector — что клиент уже видел.

```json
{
  "type": "sync_start",
  "device_id": "delphi-android-550e8400-e29b-41d4",
  "device_name": "Delphi Android",
  "platform": "android",
  "vector": {
    "delphi-android-550e8400": 42,
    "delphi-web-abc123": 17
  }
}
```

**Поля:**
- `device_id` — уникальный идентификатор устройства (генерируется при первом запуске)
- `device_name` — человекочитаемое имя
- `platform` — `"android"` / `"ios"` / `"macos"` / `"web"` / `"server"`
- `vector` — словарь `{device_id: last_seen_seq}`, то что клиент уже получил

---

### 2. `sync_changes` — сервер → клиент

Ответ на `sync_start`. Сервер отдаёт все изменения которые клиент пропустил.

```json
{
  "type": "sync_changes",
  "changes": [
    {
      "event_id": "550e8400-e29b-41d4-a716-446655440000",
      "change_type": "create",
      "data": {
        "event_type": "task",
        "category": "productivity",
        "source": "delphi-android",
        "source_id": "550e8400-e29b-41d4-a716-446655440000",
        "summary": "Купить молоко",
        "occurred_at": "2026-03-30T10:00:00.000Z",
        "data": {
          "id": "550e8400-e29b-41d4-a716-446655440000",
          "title": "Купить молоко",
          "priority": 1,
          "isToday": true,
          "isCompleted": false,
          "isCancelled": false,
          "isTrashed": false,
          "sortOrder": 0,
          "createdAt": "2026-03-30T10:00:00.000Z"
        }
      },
      "device_id": "delphi-android-550e8400",
      "device_seq": 43
    }
  ]
}
```

---

### 3. `change` — realtime мутация (в обе стороны)

Клиент → Сервер: новое изменение в реальном времени.
Сервер → Другие клиенты: broadcast этого изменения.

```json
{
  "type": "change",
  "event_id": "550e8400-e29b-41d4-a716-446655440000",
  "change_type": "create",
  "data": { ... },
  "device_id": "delphi-android-550e8400",
  "device_seq": 44
}
```

**`change_type`:** `"create"` / `"update"` / `"delete"`

---

### 4. `change_ack` — сервер → клиент

Подтверждение после `change`. Клиент должен обновить свой version vector.

```json
{
  "type": "change_ack",
  "event_id": "550e8400-e29b-41d4-a716-446655440000",
  "device_seq": 44
}
```

---

### 5. `ping` / `pong` — keepalive

Сервер каждые 30 секунд:
```json
{"type": "ping"}
```
Клиент отвечает:
```json
{"type": "pong"}
```

---

## Операции с задачами

### Создать задачу

```json
{
  "type": "change",
  "event_id": "550e8400-e29b-41d4-a716-446655440000",
  "change_type": "create",
  "device_id": "delphi-android-abc",
  "device_seq": 1,
  "data": {
    "event_type": "task",
    "category": "productivity",
    "source": "delphi-android",
    "source_id": "550e8400-e29b-41d4-a716-446655440000",
    "summary": "Название задачи",
    "occurred_at": "2026-03-30T12:00:00.000Z",
    "data": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "title": "Название задачи",
      "notes": "Описание (опционально)",
      "priority": 2,
      "scheduledDate": "2026-03-31",
      "isToday": false,
      "isEvening": false,
      "isSomeday": false,
      "isCompleted": false,
      "isCancelled": false,
      "isTrashed": false,
      "sortOrder": 0,
      "createdAt": "2026-03-30T12:00:00.000Z"
    }
  }
}
```

---

### Обновить задачу (любые поля)

```json
{
  "type": "change",
  "event_id": "550e8400-e29b-41d4-a716-446655440000",
  "change_type": "update",
  "device_id": "delphi-android-abc",
  "device_seq": 2,
  "data": {
    "event_type": "task",
    "category": "productivity",
    "source": "delphi-android",
    "source_id": "550e8400-e29b-41d4-a716-446655440000",
    "summary": "Новое название",
    "occurred_at": "2026-03-30T13:00:00.000Z",
    "data": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "title": "Новое название",
      "priority": 3,
      "scheduledDate": "2026-04-01",
      "isToday": true,
      "isCompleted": false,
      "isCancelled": false,
      "isTrashed": false,
      "sortOrder": 0,
      "createdAt": "2026-03-30T12:00:00.000Z"
    }
  }
}
```

> **Upsert по `source + source_id`:** Ark делает идемпотентный upsert.
> Повторная отправка того же `source_id` обновит запись, не создаст дубль.

---

### Завершить задачу

```json
{
  "type": "change",
  "event_id": "550e8400-...",
  "change_type": "update",
  "data": {
    "event_type": "task",
    "source": "delphi-android",
    "source_id": "550e8400-...",
    "summary": "Название задачи",
    "occurred_at": "2026-03-30T14:00:00.000Z",
    "data": {
      "id": "550e8400-...",
      "title": "Название задачи",
      "isCompleted": true,
      "completedAt": "2026-03-30T14:00:00.000Z",
      "isCancelled": false,
      "isTrashed": false,
      "sortOrder": 0,
      "createdAt": "2026-03-30T12:00:00.000Z"
    }
  }
}
```

---

### Отменить задачу

```json
{
  "data": {
    "data": {
      "isCancelled": true,
      "cancelledAt": "2026-03-30T14:00:00.000Z",
      "isCompleted": false,
      "isTrashed": false
    }
  }
}
```

---

### Переместить в корзину

```json
{
  "data": {
    "data": {
      "isTrashed": true,
      "isCompleted": false,
      "isCancelled": false
    }
  }
}
```

---

### Удалить задачу навсегда

```json
{
  "type": "change",
  "event_id": "550e8400-...",
  "change_type": "delete",
  "data": {
    "event_type": "task",
    "source": "delphi-android",
    "source_id": "550e8400-..."
  }
}
```

---

## Version Vector

Каждый клиент хранит словарь `{device_id: last_seen_seq}`.

**Обновление:**
- После `change_ack`: `vector[device_id] = device_seq`
- После `sync_changes`: `vector[change.device_id] = max(vector[change.device_id], change.device_seq)` для каждого change

**При переподключении:** отправляем актуальный вектор в `sync_start`.
Сервер сам вычислит какие изменения нужно отдать.

**Сброс (resync):** очистить вектор → `sync_start` с пустым `vector: {}` → сервер отдаст все изменения.

---

## Дедупликация

Ark использует `source + source_id` как уникальный ключ:

- `source` = `"delphi-android"` / `"delphi-web"` / `"delphi"` (macOS)
- `source_id` = UUID задачи

Повторная отправка одной и той же задачи — безопасный upsert, дублей не будет.

---

## Офлайн / Outbox

1. Пока нет подключения — изменения сохраняются в локальный outbox
2. При reconnect — отправляем `sync_start`
3. Получаем пропущенные изменения от сервера (`sync_changes`)
4. Отправляем накопленный outbox как batch или по одному (`change`)

---

## Smart Lists — как фильтровать

| Список | Фильтр по полям задачи |
|--------|----------------------|
| Входящие | нет `projectId`, `isToday=false`, `isSomeday=false`, `isCompleted=false`, `isTrashed=false` |
| Сегодня | `isToday=true` или `scheduledDate=сегодня`, `isCompleted=false` |
| Планы | `scheduledDate` в будущем, `isToday=false`, `isSomeday=false`, `isCompleted=false` |
| Потом | `isSomeday=true`, `isCompleted=false` |
| Журнал | `isCompleted=true` или `isCancelled=true` |
| Корзина | `isTrashed=true` |

---

## Env переменные сервера

| Переменная | Описание |
|-----------|----------|
| `LIFE_API_KEY` | API ключ (используется как `key=` в URL и как HMAC secret для P2P) |
| `LIFE_DB_PATH` | Путь к SQLite БД |
| `ARK_PORT` | Порт сервера (по умолчанию 8000) |
| `ARK_PEER_URLS` | Comma-separated WebSocket URLs удалённых Ark-инстансов для авто-пиринга |
| `ARK_MDNS` | `"0"` чтобы отключить mDNS |
| `ARK_DEVICE_ID` | ID этого Ark-сервера в mesh |
| `ARK_DEVICE_NAME` | Имя для mDNS (по умолчанию hostname) |

---

## P2P Пиринг между Ark-инстансами

Несколько Ark-серверов (например Mac и VPS) синхронизируются напрямую через `/peer/sync`.

**Запуск Mac Ark с авто-пирингом к VPS:**
```bash
ARK_PEER_URLS=ws://vps.example.com:8000 \
LIFE_API_KEY=your-key \
LIFE_DB_PATH=ark.db \
uvicorn server.app:app --host 0.0.0.0 --port 8000
```

**Протокол `/peer/sync`:**
1. `peer_hello` с HMAC-SHA256 (ключ = `LIFE_API_KEY`)
2. `peer_hello_ack` — взаимная аутентификация
3. `sync_start` с version vector
4. `sync_changes` — пропущенные изменения
5. Realtime: `peer_change` с полем `hop_path` для предотвращения петель

**LAN Discovery на Android:**
Android ищет `_ark-sync._tcp` сервисы через NSD (DNS-SD).
При нахождении локального Ark переключается на него, при потере — падбэк на remote URL.
