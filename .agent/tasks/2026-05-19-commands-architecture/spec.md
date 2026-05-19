# 2026-05-19 — Commands architecture (V1 manifest + V2 dynamic auto-launch)

## Goal

Переход от hardcoded extension-команд в `shell/electron/commands.ts` к
**трёхслойной** архитектуре:

1. **Kepler-internal** (settings/dashboard/check-updates) — остаются в shell.
2. **Manifest-declared** — extension объявляет команды в `manifest.json`.
   Видны в launcher всегда (пока extension установлен), не зависят от
   running state. Invoke → openExtension(id, route).
3. **Runtime dynamic** — `commands.register(...)` через WS из running
   extension'а. Для state-aware actions (Stop pomodoro и т.п.). При
   invoke action-команды, чей extension не запущен — Kepler **auto-launch**'ит
   extension и dispatch'ит invoke после mount.

## Acceptance Criteria

### AC1 — Manifest schema + loader → PASS если

- `KextManifest` имеет опциональное поле `commands?: KextCommand[]`.
- `KextCommand`: `{ id, title, subtitle?, icon?, route?, kind?, mode?: "open" | "action" }`.
- Полный id = `${manifest.id}:${cmd.id}` (security boundary).
- `loadDeclaredCommands()` в extension-host сканирует все установленные
  + dev-tree extension'ы, читает manifests, билдит `CommandRecord[]`.
- Icon path резолвится относительно extension dir → data URI (как
  `extensionIconDataUri` для top-level icon).

### AC2 — IPC integration → PASS если

- `ipcMain.handle("kepler:commands:list")` мержит:
  1. static internal (settings/dashboard/check-updates),
  2. manifest-declared,
  3. dynamic registered (через ARK commands.list),
  4. dedup по id, internal > manifest > dynamic priority.
- `ipcMain.handle("kepler:commands:invoke", id)` resolves:
  - internal → exec
  - manifest `mode: "open"` → openExtension(extId, route)
  - manifest `mode: "action"` / dynamic → arkClient.commands.invoke(id)
    с auto-launch fallback (см. AC4)
- При `extension:install/uninstall` shell бродкастит
  `kepler:commands:updated` → launcher refresh.

### AC3 — Migration Eden/Horologion/Delphi → PASS если

- `extensions/eden/manifest.json` имеет `commands: [open, note:create, note:open-today]`
  с icon-path.
- `extensions/horologion/manifest.json` имеет
  `commands: [open, pomodoro, stopwatch]`.
- `extensions/delphi/manifest.json` имеет `commands: [open, inbox]`.
- `shell/electron/commands.ts` — только internal kepler-команды
  (`settings:open`, `dashboard:open`, `kepler:check-updates`).
  Extension-specific entries удалены.
- `LauncherView.vue` BuiltInIcon mapping для extension id'шников
  удалён — icon приходит из manifest как data URI.

### AC4 — Auto-launch для dynamic action → PASS если

- Когда invoke action-команды (id содержит `:` extension id'шник),
  extension не запущен, kepler:
  1. openExtension(extId) без route,
  2. ждёт до 5 сек пока extension mount + commands.register отработает,
  3. dispatch'ит invoke через ARK commands.invoke,
  4. error если не дождался mount.
- При наличии running extension — invoke сразу.

### AC5 — Edge cases → PASS если

- Uninstall extension убирает его команды из launcher (refresh
  при `extension:uninstall` IPC).
- Install extension с manifest.commands добавляет команды в launcher.
- Команда с duplicate id (например внутренняя collision) — internal wins.
- Empty/missing manifest.commands не падает.
- Невалидный icon path → fallback на extension top-level icon.
- Тест "вызов команды от unknown extension'а" → graceful error.

### AC6 — Tests → PASS если

- Unit: `loadDeclaredCommands` returns корректный CommandRecord[].
- Unit: manifest validator отвергает невалидный `commands` block
  (wrong id format, missing title).
- E2E (`tests/e2e/commands-architecture.spec.ts`):
  - launcher показывает manifest-declared commands.
  - invoke `eden:open` → Eden window появляется.
  - invoke `eden:note:open-today` → Eden открыт + zen mode active +
    journal entry создан.
  - Auto-launch: invoke dynamic action команды Eden (если задана)
    когда Eden не запущен → Eden автоматически открыт + action выполнен.
  - Uninstall Eden → команды Eden исчезают из commands.list.

### AC7 — Docs → PASS если

- `docs-site/concepts/command-bus.md` обновлён с тремя слоями.
- `docs-site/agents/forbidden.md`: запрет hardcoded extension commands
  в shell.
- `docs:sync` + `docs:check` чисто.

### AC8 — Guards → PASS если

- `bun run ark:guard:writes` ✓
- `bun typecheck shell` ✓

## Out of scope

- Preferences в manifest (Raycast-style settings page auto-render) — V3.
- Command parameters / arguments в launcher UI — V3.
- Per-command icons с tint / colorClass — V3 (пока только path).
- Signed manifests / third-party security review — V3.

## Estimate

~3-4 часа на полную реализацию + tests + docs.
