SQLite база данных которая собирает в себе данные вообще обо всем на свете

### Типы
#### Входящее
Текстовая заметка без определённого типа.
- Название*
- Описание
#### Задача
Любая задача.
- Название*
- Статус*
- Описание
- Чек-лист
- Целевая дата
- Время от - до
- Напоминание (за какое время напомнить)

# Life DB

Личная база данных на вашем VPS: любые приложения (todo, трекеры, скрипты) читают/пишут данные через HTTP API.

- Один общий ключ доступа (API key).
- Все время хранится в UTC-0, UI показывает локальное время пользователя.
- Swagger/OpenAPI доступен на `/docs`.

## Структура проекта

```
./
├── core/       # Python библиотека (SQLite + схема + тесты)
├── server/     # HTTP API (FastAPI) + Swagger
├── ui/         # Web UI (Svelte). После сборки раздается сервером
├── plugins/    # Импортеры (например, Toggl Track)
└── examples/   # Примеры/демо базы
```

## Проверка локально (с тестовой БД)

1) Сгенерируйте демо-базу:

```bash
python3 core/generate_demo_db.py --output examples/demo.db --overwrite --count 500
```

2) Соберите web UI (чтобы сервер мог его раздавать):

```bash
cd ui
bun install
bun run build
cd ..
```

3) Поднимите сервер:

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r server/requirements.txt

export LIFE_DB_PATH="$PWD/examples/demo.db"
export LIFE_API_KEY="change-me-to-a-long-random-secret"

uvicorn server.app:app --host 127.0.0.1 --port 8000
```

Откройте:
- UI: `http://127.0.0.1:8000/`
- Swagger: `http://127.0.0.1:8000/docs`

4) Подключите UI:
- Нажмите «Подключиться»
- Укажите `http://127.0.0.1:8000`
- Вставьте ваш `LIFE_API_KEY`

## Плагины (пример: Toggl Track)

Плагины пишут события в файл БД напрямую через `core` (подойдет для бэкенд-скриптов/cron).

```bash
python3 -m plugins.sync_toggl --full --output examples/toggl_track.db
```
