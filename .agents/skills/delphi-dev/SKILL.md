---
name: delphi-dev
description: Запустить Delphi Vue-extension в dev/HMR режиме внутри Kosmos desktop shell. Use when user asks to run Delphi desktop development UI.
disable-model-invocation: true
allowed-tools: Bash
---

Запусти Delphi desktop как Vue-extension, а не standalone Electron app.

1. Для HMR dev server только Delphi extension:

```bash
rtk bun run --cwd platform/desktop dev:extensions:only delphi
```

2. Если нужен полный Kosmos shell вместе с backend/runtime:

```bash
rtk bun run --cwd platform/desktop dev
```

3. Ожидаемый dev port Delphi из `products/delphi/manifest.json`: `http://localhost:5182/`.
4. Сообщи пользователю, какой режим запущен и где смотреть ошибки.
