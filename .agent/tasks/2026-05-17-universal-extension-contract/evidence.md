# Evidence — 2026-05-17 universal-extension-contract (Loop B)

Verified at: 2026-05-17

## AC1 — ExtensionManifest.tests field

**PASS.**

`shell/electron/extension-host.ts` теперь содержит:

```ts
tests?: {
  commands?: string[];
  smoke?: {
    objectType: string;
    sample?: {
      title?: string;
      content?: unknown;
      props?: Record<string, unknown>;
    };
  };
};
```

С полным doc-комментом (см. файл).

## AC2 — Все 4 extension manifest'а имеют tests блок

**PASS.**

```
$ cat extensions/eden/manifest.json | jq .tests
{ "commands": ["eden:note:create","eden:note:search"],
  "smoke": { "objectType": "note_obj", "sample": { "title": "contract-smoke-eden" } } }

$ cat extensions/horologion/manifest.json | jq .tests
{ "commands": ["horologion:pomodoro:25","horologion:pomodoro:50","horologion:stopwatch:start"],
  "smoke": { "objectType": "time_entry_obj", "sample": { "title": "contract-smoke-horologion" } } }

$ cat extensions/delphi/manifest.json | jq .tests
{ "commands": ["delphi:task:create","delphi:task:today"],
  "smoke": { "objectType": "task_obj", "sample": { "title": "contract-smoke-delphi" } } }

$ cat extensions/arrancador/manifest.json | jq .tests
{ "commands": [],
  "smoke": { "objectType": "game_obj", "sample": { "title": "contract-smoke-arrancador" } } }
```

## AC3 — tests/e2e/extensions-contract.spec.ts

**PASS.**

`tests/e2e/extensions-contract.spec.ts` создан:

- Discover'ит `extensions/*/manifest.json` через `fs.readdirSync` + `JSON.parse`.
- Для каждого с `tests` блоком — `test.describe("extension contract: ${id}")` с одним test'ом.
- Test: открывает extension через `kepler.commands.invoke('${id}:open')` из launcher window'а, ждёт extension window, проверяет:
  - `document.readyState === "complete"` (extension живой)
  - `commands.list` через `extWindow.evaluate(kepler.ark.request)` содержит все объявленные `tests.commands`
  - ARK round-trip: `upsert_object` с sample → `get_object` → `delete_object` cleanup, assert id/typeId/title

Playwright обнаруживает 4 contract test'а:

```
$ bunx playwright test --list | grep contract
extensions-contract.spec.ts:68:5 › extension contract: arrancador › arrancador: загружается + commands + ARK round-trip
extensions-contract.spec.ts:68:5 › extension contract: delphi › delphi: загружается + commands + ARK round-trip
extensions-contract.spec.ts:68:5 › extension contract: eden › eden: загружается + commands + ARK round-trip
extensions-contract.spec.ts:68:5 › extension contract: horologion › horologion: загружается + commands + ARK round-trip
```

См. `raw/playwright-list.log`.

## AC4 — tests/e2e/eden.spec.ts

**PASS.**

`tests/e2e/eden.spec.ts` создан с двумя test'ами:

1. **`eden: shim installed, save/list/delete entry round-trip`** — открывает Eden, проверяет `window.api` shim установлен, saveEntry → listEntries → deleteEntry round-trip с уникальным id.
2. **`eden: TipTap editor mountains после открытия заметки`** — открывает Eden, создаёт заметку через shim, ожидает render Eden UI.

Playwright detects:

```
eden.spec.ts:50:3 › eden extension › eden: shim installed, save/list/delete entry round-trip
eden.spec.ts:128:3 › eden extension › eden: TipTap editor mountains после открытия заметки
```

## AC5 — typecheck зелёный

**PASS.**

```
$ bun run --cwd shell typecheck
$ tsc --noEmit
(0 errors)
```

См. `raw/typecheck-1.log`.

## AC6 — build:js собирается

**PASS.**

```
$ bun run --cwd shell build:js
... 7 builds ✓
```

См. `raw/build-1.log`. Renderer + main process + все 4 extensions собрались.

## AC7 — Manifest deserialize не падает на наличии `tests`

**PASS.**

`loadExtensionManifest()` в `shell/electron/extension-host.ts` использует `JSON.parse` — TypeScript структурная типизация (`as ExtensionManifest`) безразлична к наличию дополнительных полей. Test field optional, поэтому extensions без `tests` (если такие появятся) тоже валидно загружаются. Build:js собрался — значит manifest parsing типобезопасен.

## AC8 — Прогон e2e

**PENDING_OPERATOR.**

Не могу запустить `bun run test:e2e --grep "extension contract|eden"` из-за pre-existing stale ACL lock-файлов в `tests/.e2e/*/kepler.lock.json` (дата 15 мая, ACL deny от прошлого username). Подробности — `2026-05-17-eden-extension/problems.md` пункт 2.

Оператору после cleanup'а нужно прогнать:

```powershell
bun run test:e2e --grep "extension contract"  # 4 contract test'а
bun run test:e2e --grep "eden"                # 2 eden test'а
bun run test:e2e                              # полный suite (20 test'ов всего)
```

И зафиксировать результаты в `raw/playwright-final.log`. Если test'ы fail'ят — fix через `problems.md` + reverify.

## Verification summary

| AC                                | Verdict                                     |
| --------------------------------- | ------------------------------------------- |
| AC1 ExtensionManifest.tests field | PASS                                        |
| AC2 4 manifest'а с tests          | PASS                                        |
| AC3 extensions-contract.spec.ts   | PASS (defined + discovered)                 |
| AC4 eden.spec.ts                  | PASS (defined + discovered)                 |
| AC5 typecheck                     | PASS                                        |
| AC6 build:js                      | PASS                                        |
| AC7 manifest parsing не падает    | PASS (build + typecheck зелёные)            |
| AC8 e2e run                       | PENDING_OPERATOR (ACL lock-файлы блокируют) |
