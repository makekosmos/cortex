# Ark — Life DB

Персональная SQLite база данных. Хранит всё: задачи, питание, здоровье, финансы, медиа — через единое event API.

## Стек

- **Core**: Python (zero deps), SQLite + FTS5
- **Server**: FastAPI + Uvicorn
- **Sync**: WebSocket + mDNS (zeroconf)
- **UI**: Svelte (опционально, после сборки раздаётся сервером)

## Структура

```
core/
  ark.py              # Класс Ark — основной интерфейс к БД (~1050 строк)
  sync.py             # SyncManager — version vectors, outbox, конфликты
  schema.sql          # Схема SQLite (events, entities, sync tables, FTS5)
  cli.py              # CLI: ark add/task/list/search/stats/sync/serve/pair
  __main__.py         # python -m core.cli entry point
  tests/
    test_ark.py       # 60 тестов
    test_sync.py      # 28 тестов
server/
  app.py              # FastAPI HTTP API + Swagger (/docs)
  sync_ws.py          # WebSocket /ws/sync — realtime sync
  discovery.py        # mDNS broadcast/discover (_ark-sync._tcp.local.)
  pairing.py          # Pairing codes (ark-XXXX) + QR генерация
  requirements.txt    # fastapi, uvicorn, websockets, zeroconf, qrcode
seed_delphi.py          # Seed 70 тестовых задач (10 на каждый GTD-тип: inbox, today, upcoming, someday, completed, cancelled, trashed)
plugins/
  toggl_track.py      # Импорт из Toggl Track
  elysium.py          # Импорт из Elysium (питание/вода)
  sync_toggl.py       # CLI: python -m plugins.sync_toggl
  sync_elysium.py     # CLI: python -m plugins.sync_elysium
```

## Быстрый старт

```bash
cd packages/ark
python3 -m venv .venv && source .venv/bin/activate
pip install -r server/requirements.txt

# Демо-БД
python3 core/generate_demo_db.py --output examples/demo.db --overwrite

# Сервер
LIFE_DB_PATH=examples/demo.db LIFE_API_KEY=dev-test-key \
  uvicorn server.app:app --host 0.0.0.0 --port 8000

# CLI
LIFE_DB_PATH=examples/demo.db python3 -m core.cli stats
LIFE_DB_PATH=examples/demo.db python3 -m core.cli task "Купить кефир"
```

## Sync протокол

Полное описание в SYNC.md. Здесь — что нужно знать при разработке.

### WebSocket endpoint

```
ws://host:8000/ws/sync?key=<LIFE_API_KEY>
```

Реализация сервера: `server/sync_ws.py` → `websocket_sync()`.

### Последовательность подключения

```
1. Client → Server:  {"type": "sync_start", "device_id": "...", "platform": "...", "vector": {...}}
2. Server → Client:  {"type": "sync_changes", "changes": [...], "server_epoch": "<uuid>", "is_full_sync": bool}   ← пропущенные изменения
3. Client → Server:  {"type": "change", ...}                       ← realtime мутации
4. Server → Others:  {"type": "change", ...}                       ← broadcast всем кроме отправителя
5. Server → Client:  {"type": "change_ack", "event_id": "...", "device_seq": 42}
6. Server → Client:  {"type": "ping"}  (каждые 30с)
7. Client → Server:  {"type": "pong"}
```

- `server_epoch` — UUID из таблицы `sync_meta`, генерируется при создании БД. Клиенты детектят wipe сервера при смене epoch.
- `is_full_sync` — `true` когда клиент прислал пустой version vector (первый sync или после reset).

### Структура change-сообщения

```json
{
  "type": "change",
  "event_id": "<uuid>",
  "change_type": "create" | "update" | "delete",
  "device_id": "delphi-web-abc",
  "device_seq": 43,
  "data": {
    "event_type": "task" | "project" | "area" | "tag",
    "category": "productivity",
    "source": "delphi-web" | "delphi" | "delphi-android",
    "source_id": "<uuid объекта>",
    "summary": "Заголовок",
    "occurred_at": "2025-01-01T12:00:00Z",
    "data": { /* произвольные поля объекта */ }
  }
}
```

### Как Ark хранит события

`_apply_to_events()` в `sync_ws.py` делает upsert в таблицу `events` по `source_id` (без `source`). Это предотвращает дубликаты, когда разные клиенты отправляют один объект с разным `source`.
- `create` / `update` → upsert (idempotent)
- `delete` → `is_deleted = 1`

Структура таблицы `events` — см. `core/schema.sql`.

### Как получить данные через CLI

```bash
# Задачи
LIFE_DB_PATH=examples/demo.db .venv/bin/python -m core.cli list --type task

# Все события
LIFE_DB_PATH=examples/demo.db .venv/bin/python -m core.cli list
```

### Pairing (подключение нового устройства)

```bash
# Генерировать QR + код
LIFE_API_KEY=... LIFE_DB_PATH=... .venv/bin/python -m core.cli pair
```

REST: `POST /pairing/create` → код `ark-XXXX` → `POST /pairing/claim` (клиент вводит код).

## Тесты

```bash
.venv/bin/python -m pytest core/tests/ server/ -v
```

## Env переменные

| Переменная | Описание | По умолчанию |
|-----------|----------|-------------|
| LIFE_DB_PATH | Путь к SQLite БД | ./ark.db |
| LIFE_API_KEY | API ключ для авторизации | (обязательный) |
| ARK_MDNS | Включить mDNS ("0" для отключения) | "1" |
| ARK_PORT | Порт сервера | 8000 |
| ARK_DEVICE_NAME | Имя устройства для mDNS | hostname |

## Conventions

- Класс называется `Ark` (не Dataverse)
- Все timestamps в UTC (ISO 8601)
- source_id для дедупликации (idempotent upsert по source_id alone)
- Soft delete (is_deleted flag)
- Event data — произвольный JSON
