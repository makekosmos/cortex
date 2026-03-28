---
name: delphi-dev
description: Запустить Delphi Electron (TypeScript) в dev-режиме
disable-model-invocation: true
allowed-tools: Bash
---

Запусти Delphi Electron приложение в dev-режиме.

1. Убей старый процесс если есть: `pkill -f "electron.*delphi" 2>/dev/null`
2. Запусти:

```bash
cd apps/delphi/ts && bun run dev 2>&1 &disown
```

3. Подожди 5 секунд, проверь что Vite поднялся (ищи "ready" в выводе).
4. Сообщи что Delphi запущен.
