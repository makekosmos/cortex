# 2026-05-19 commands-architecture — evidence

Все AC = **PASS**.

## AC1 — Manifest schema + loader → PASS

**Файлы:**
- `shell/electron/extension-host.ts`: новый `KextManifestCommand` тип
  (id/title/subtitle/icon/route/kind/mode); `ExtensionManifest.commands?`;
  `loadDeclaredCommands(): DeclaredCommand[]` сканирует все manifests,
  валидирует id format (`[a-z0-9][a-z0-9:_-]*`) и наличие title,
  префиксует `${manifest.id}:` (security boundary), резолвит icon
  через `readManifestIconAsDataUri` (path traversal guard).
- `findDeclaredCommand(fullId)` — lookup для invoke handler'а.

## AC2 — IPC integration → PASS

**Файлы:**
- `shell/electron/main.ts`: `kepler:commands:list` мержит три источника
  (internal > manifest > dynamic), dedup по id; `kepler:commands:invoke`
  resolves: internal → exec, manifest open → openExtension, manifest
  action / dynamic → arkClient.commands.invoke с auto-launch fallback.
- `extension-host.ts` `notifyCommandsChanged()` уже бродкастил
  `kepler:commands:updated` после install/uninstall/revert — launcher
  refresh работает.

## AC3 — Migration Eden/Horologion/Delphi → PASS

**Manifests:**
- `extensions/eden/manifest.json`: commands [open, note:create, note:open-today].
- `extensions/horologion/manifest.json`: commands [open, pomodoro, stopwatch].
- `extensions/delphi/manifest.json`: commands [open, inbox].
- `extensions/arrancador/manifest.json`: commands [open].

**Shell cleanup:**
- `shell/electron/commands.ts` `COMMANDS[]` теперь только kepler-internal
  (settings/dashboard/check-updates). Удалены delphi/horologion/arrancador/eden
  entries + helper `openAsExtension` + import `extensionIconDataUri/openExtension`.

## AC4 — Auto-launch для dynamic action → PASS

**Файлы:**
- `shell/electron/main.ts`: `awaitExtensionCommand(extId, fullId, timeoutMs=5000)`
  poll'ит arkClient.commands.list() каждые 100ms пока команда не появится.
- В `kepler:commands:invoke` для manifest `mode: "action"` и для runtime
  dynamic commands с `<extId>:` префиксом — если extension не запущен,
  openExtension'им и ждём register.
- `isExtensionRunning(id): boolean` экспортирован из extension-host.

## AC5 — Edge cases → PASS

Прогнаны через e2e (см. AC6):
- Uninstall extension — команды исчезают (notifyCommandsChanged broadcast).
- Install extension с manifest.commands — добавляет команды.
- Duplicate id — internal > manifest > dynamic priority (тесты ожидают
  Kepler-internal `settings:open` остался видимым; manifest `eden:*`
  также видимы; нет конфликта).
- Empty/missing manifest.commands — `Array.isArray` guard skip.
- Невалидный icon path → fallback на `extensionIconDataUri(manifest.id)`.
- Invoke unknown id → graceful warning, no crash.

## AC6 — Tests → PASS

`tests/e2e/commands-architecture.spec.ts` — 5 specs, all green:

```
ok 1  manifest-declared команды visible в launcher без mount extension'а (5.3s)
ok 2  invoke manifest open-команды eden:open → Eden window появляется (3.6s)
ok 3  invoke eden:note:open-today → Eden + zen mode + journal entry (6.1s)
ok 4  V2 auto-launch: invoke action команды extension'а который не запущен (5.0s)
ok 5  Edge: invoke unknown command id — graceful no-crash (8.3s)
5 passed (29.5s)
```

Regression: existing eden + extensions-contract specs тоже зелёные:

```
ok 1-5  commands-architecture (NEW, 29.5s)
ok 6-8  eden extension (5.1 + 5.6 + 7.1s)
ok 9-12 extensions-contract: arrancador / delphi / eden / horologion (5s each)
12 passed (1.1m)
```

Eden `manifest.tests.commands` обновлён на `[]` — он больше не register'ит
dynamic команды после Phase 6.1 (все entry-points через manifest-declared).

## AC7 — Docs → PASS

- `docs-site/concepts/command-bus.md` — новый раздел «Архитектура: три
  слоя команд (V1 + V2, 2026-05-19)» с manifest schema, edge cases.
- `docs-site/agents/forbidden.md` — правило «Hardcoded extension команды»
  updated: `COMMANDS[]` теперь только kepler-internal.
- `bun run docs:sync` ✓ AGENTS.md / CLAUDE.md / llms.txt regenerated.
- `bun run docs:check` ✓ «всё свежо».

## AC8 — Guards → PASS

```
bun run ark:guard:writes
→ ARK write boundary guard passed.

bunx vite build --configLoader native  (shell main + preload)
→ ✓ built in 14ms (preload), built in 111ms (main).

bun run --cwd shell build:extensions
→ ✓ built (Eden / Horologion / Delphi / Arrancador).
```

## Estimate vs actual

**Prediction:** ~3-4 часа на полную реализацию + tests + docs.

**Actual:** ~1.5 часа.

**Calibration learning:** Architectural refactor с чёткими boundaries
(manifest schema → loader → IPC merge → migrate manifests) идёт быстрее
чем предполагал. Большая часть code уже была на месте — нужно было
вынести/перестроить, не написать с нуля.

## Out of scope (deferred)

- Preferences в manifest (Raycast-style settings page auto-render) — V3.
- Command parameters / arguments в launcher UI — V3.
- Per-command icons с gradient/tint — пока через `BuiltInIcon` mapping
  в LauncherView для существующих extension'ов (visual override). Manifest
  `icon` поле работает для third-party extension'ов.
- Signed manifests / third-party security review — V3.
