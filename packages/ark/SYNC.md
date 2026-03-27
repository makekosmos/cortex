# Ark Sync Protocol

Синхронизация данных между устройствами в экосистеме Kosmos.

## Топология

```
             VPS (Финляндия)
             Ark relay hub
            /      |      \
         Mac    Android   Windows
       (mobile) (mobile)  (home WiFi)
```

- **VPS** — всегда онлайн, relay для всех устройств
- Устройства не имеют открытых портов
- Через интернет — только через VPS
- На одной WiFi — прямой P2P через mDNS

## Транспорт

### WebSocket (основной)
Каждое устройство подключается к VPS по WebSocket (`wss://ark.example.com/ws/sync`).
Соединение persistent — сервер пушит изменения в реальном времени.

### mDNS (LAN)
Устройства в одной сети обнаруживают друг друга через mDNS.
Если найден локальный Ark — синк идёт напрямую, быстрее и без VPS.

## Версионирование

### Version Vector (часы устройств)
Каждое устройство имеет уникальный `device_id`.
Каждая запись хранит:

```
device_id   — кто последний менял
device_seq  — монотонный счётчик устройства (инкремент при каждой записи)
updated_at  — wall-clock timestamp (для UI и last-write-wins)
```

Каждое устройство хранит **version vector** — словарь `{device_id: last_seen_seq}`:
```json
{"mac-abc": 142, "android-def": 87, "win-xyz": 23}
```

### Протокол синхронизации

**Подключение:**
```
Client → Server: {
  "type": "sync_start",
  "device_id": "android-def",
  "vector": {"mac-abc": 140, "android-def": 87, "win-xyz": 20}
}

Server → Client: {
  "type": "sync_changes",
  "changes": [... все изменения которых нет у клиента ...]
}

Client → Server: {
  "type": "sync_changes",
  "changes": [... все локальные изменения которых нет на сервере ...]
}
```

**Realtime (после начальной синхронизации):**
```
При локальном изменении:
  Client → Server: {"type": "change", "data": {...}}
  Server → All other clients: {"type": "change", "data": {...}}
```

## Конфликты

### Когда возникают
Два устройства редактируют одну и ту же запись офлайн.
Например: Mac и Android оба меняют заголовок задачи пока нет связи.

### Обнаружение
При мерже: если запись с тем же `source + source_id` пришла с двух устройств
и оба `device_seq` новее чем последний известный — конфликт.

### Разрешение

**Автоматическое (по умолчанию):**
Last-write-wins по `updated_at`. Работает для 99% случаев.

**Ручное (для важных данных):**
Конфликтная запись сохраняется в таблицу `sync_conflicts`:
```sql
CREATE TABLE sync_conflicts (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,         -- какое событие в конфликте
    local_data TEXT NOT NULL,       -- JSON локальной версии
    remote_data TEXT NOT NULL,      -- JSON удалённой версии
    local_device TEXT NOT NULL,
    remote_device TEXT NOT NULL,
    local_updated_at TEXT NOT NULL,
    remote_updated_at TEXT NOT NULL,
    resolved INTEGER DEFAULT 0,     -- 0 = pending, 1 = resolved
    resolution TEXT,                -- 'local', 'remote', 'manual'
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

**CLI для разрешения:**
```bash
# Посмотреть конфликты
python3 -m ark.sync conflicts

# Разрешить
python3 -m ark.sync resolve <conflict_id> --keep local
python3 -m ark.sync resolve <conflict_id> --keep remote
```

## Таблицы синхронизации (дополнение к schema.sql)

```sql
-- Какие устройства известны
CREATE TABLE sync_devices (
    device_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,              -- "MacBook", "Pixel 8", "Windows PC"
    platform TEXT NOT NULL,          -- "macos", "android", "windows"
    last_seen_at TEXT,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Version vector: что каждое устройство видело от других
CREATE TABLE sync_vectors (
    device_id TEXT NOT NULL,         -- чей вектор
    peer_id TEXT NOT NULL,           -- от кого видели
    last_seq INTEGER NOT NULL,       -- до какого seq видели
    PRIMARY KEY (device_id, peer_id)
);

-- Очередь неотправленных изменений (для offline)
CREATE TABLE sync_outbox (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL,
    change_type TEXT NOT NULL,       -- 'create', 'update', 'delete'
    data TEXT NOT NULL,              -- JSON полного события
    device_id TEXT NOT NULL,
    device_seq INTEGER NOT NULL,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

## Сценарии

### 1. Нормальная работа (все онлайн)
Mac создаёт задачу → WebSocket → VPS → WebSocket → Android видит мгновенно

### 2. Офлайн + возврат
Android офлайн → записи копятся в sync_outbox →
подключился к WiFi → mDNS находит Mac / или коннект к VPS →
отправляет outbox → получает пропущенные изменения

### 3. Конфликт
Mac офлайн меняет задачу "Купить молоко" → "Купить кефир"
Android офлайн меняет ту же задачу → "Купить молоко 2л"
Оба подключаются → VPS видит конфликт →
auto: берёт более позднее updated_at
или: сохраняет в sync_conflicts для ручного разрешения

### 4. VPS недоступен
Устройства на одной WiFi → синк через mDNS напрямую.
Устройства в разных сетях → sync_outbox копит, ждёт VPS.
VPS вернулся → всё синкается автоматически.
