---
name: ark-start
description: Запустить Ark сервер с demo DB для локальной разработки
disable-model-invocation: true
allowed-tools: Bash
---

Запусти Ark сервер для локальной разработки.

1. Проверь, не запущен ли уже (`curl -s http://localhost:8000/health`). Если да — сообщи и выйди.
2. Запусти сервер:

```bash
cd packages/ark && source .venv/bin/activate && LIFE_DB_PATH=examples/demo.db LIFE_API_KEY=dev-test-key uvicorn server.app:app --host 0.0.0.0 --port 8000 --reload 2>&1 &disown
```

3. Подожди 2 секунды, проверь health endpoint.
4. Сообщи статус: URL, API key, health.
