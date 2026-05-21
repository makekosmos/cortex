---
name: dev-all
description: Запустить всё для разработки — Ark сервер, Delphi Electron, открыть Xcode
disable-model-invocation: true
allowed-tools: Bash
---

Подними весь dev-стек Kosmos:

1. **Ark сервер** — проверь health, если не запущен — запусти:

   ```bash
   cd packages/ark && source .venv/bin/activate && LIFE_DB_PATH=examples/demo.db LIFE_API_KEY=dev-test-key uvicorn server.app:app --host 0.0.0.0 --port 8000 --reload 2>&1 &disown
   ```

2. **Delphi Electron** — убей старый процесс, запусти:

   ```bash
   cd apps/delphi/ts && bun run dev 2>&1 &disown
   ```

3. **Delphi Swift** — открой Xcode:

   ```bash
   open apps/delphi/swift/Delphi.xcodeproj
   ```

4. Подожди пока всё поднимется, проверь что Ark health OK.
5. Выведи сводку: что запущено, URL-ы, как подключиться.
