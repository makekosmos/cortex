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

См. SYNC.md для полного описания. Кратко:
- WebSocket `/ws/sync?key=API_KEY`
- Клиент шлёт `sync_start` с device_id + version vector
- Сервер отвечает пропущенными изменениями
- Realtime: изменения бродкастятся всем подключённым клиентам
- Pairing: `POST /pairing/create` → код `ark-XXXX` → `POST /pairing/claim`

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
- source/source_id для дедупликации (idempotent upsert)
- Soft delete (is_deleted flag)
- Event data — произвольный JSON
