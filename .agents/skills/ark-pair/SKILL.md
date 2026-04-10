---
name: ark-pair
description: Показать connection string и QR для подключения устройства к Ark
disable-model-invocation: true
allowed-tools: Bash
---

Покажи пользователю как подключиться к Ark:

1. Проверь что Ark сервер работает (`curl -s http://localhost:8000/health`)
2. Если не работает — сообщи что нужно сначала запустить `/ark-start`
3. Открой страницу pairing в браузере:
   ```bash
   open http://localhost:8000/pairing/qr
   ```
4. Также выведи connection string прямо в терминал:
   ```bash
   curl -s -H "X-API-Key: dev-test-key" -X POST http://localhost:8000/pairing/create | python3 -c "import sys,json; print(json.load(sys.stdin)['connection_string'])"
   ```
5. Объясни: скопируй строку `ark://...` и вставь в Settings Delphi (или любого другого приложения).
