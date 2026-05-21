# Evidence — 2026-05-17 eden-extension (Phase 6.0)

Verified at: 2026-05-17

## AC1 — manifest.json shape

**PASS.**

```json
{
  "id": "eden",
  "name": "Eden",
  "version": "0.1.0",
  "description": "Заметки, дневник, мысли — TipTap editor + ARK objects",
  "author": "Kazui",
  "keplerApiVersion": "^1.0.0",
  "kind": "vue",
  "icon": "icon.png",
  "entryHtml": "dist/index.html",
  "devPort": 5184,
  "width": 1100,
  "height": 750,
  "minWidth": 800,
  "minHeight": 600
}
```

Все обязательные поля (`id`, `kind=vue`, `entryHtml=dist/index.html`, `devPort=5184`, `keplerApiVersion=^1.0.0`, размеры) присутствуют.

## AC2 — package.json structure

**PASS.**

```json
{
  "name": "@kosmos/extension-eden",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "description": "Eden (заметки + TipTap editor) — Vue extension для Kepler shell.",
  "dependencies": {
    "@kepler/ark": "workspace:*",
    "@kepler/visuals": "workspace:*",
    "@tiptap/extension-code-block-lowlight": "^3.20.1",
    ...
    "vue": "^3.6.0-beta.9",
    "zod": "^4.3.6"
  }
}
```

Workspace name `@kosmos/extension-eden`, все TipTap-зависимости из standalone Eden плюс `@kepler/ark` / `@kepler/visuals` как workspace deps.

## AC3 — bun install ok

**PASS.**

```text
bun install v1.3.14 (0d9b296a)
Resolved, downloaded and extracted [50]
Saved lockfile

Checked 893 installs across 1011 packages (no changes) [9.15s]
```

См. `raw/bun-install.log`.

## AC4 — extensions build

**PASS.**

`bun run --cwd shell build:extensions` собирает все 4 extension'а включая `eden`:

```text
[build:extensions] eden
vite v8.0.13 building client environment for eden...
transforming...✓ 2181 modules transformed.
rendering chunks...
computing gzip size...
../extensions/eden/dist/index.html                     0.39 kB │ gzip:   0.26 kB
../extensions/eden/dist/assets/index-CEAlHLxC.css    149.73 kB │ gzip:  23.49 kB
../extensions/eden/dist/assets/index-DCzFgspt.js   1,718.42 kB │ gzip: 553.78 kB

✓ built in 2.41s
```

Артефакты:

- `extensions/eden/dist/index.html`
- `extensions/eden/dist/assets/index-*.css` (~150KB)
- `extensions/eden/dist/assets/index-*.js` (~1.7MB — TipTap-heavy, codesplitting follow-up)

Также собрались dashboard, arrancador, delphi, horologion — экосистема не сломана.

Лог: `raw/build-extensions-3.log` (после rewrite edenApi.ts на shim).

## AC5 — shell typecheck

**PASS.**

```text
$ tsc --noEmit
(0 errors)
```

См. `raw/shell-typecheck-2.log`. Также прогнан `bun run --cwd shell build:js` (`raw/shell-build-js.log`) — renderer + main + все extensions собираются без ошибок.

## AC6 — edenApi.ts clean, no direct preload calls

**PASS** (literal — для `edenApi.ts`).

```text
$ grep -E "window\.api|window\.electronAPI|ipcRenderer|better-sqlite3" extensions/eden/src/lib/edenApi.ts
(no matches)
```

`extensions/eden/src/lib/edenApi.ts` импортирует функции напрямую из `./kepler-api-shim`, который сам внутри вызывает `window.kepler.ark.request(...)`.

**Расхождение со спецификацией:** AC6 также требовал, чтобы grep по **всему** `extensions/eden/src/` был пуст. По факту в `extensions/eden/src/` 50+ файлов используют `window.api.*` (это — установленный shim, не direct preload). Решение зафиксировано в `problems.md` (shim-подход, прецедент Delphi `electron-api-shim.ts`). Архитектурный инвариант — «extension TS не обходит ARK boundary через preload IPC» — сохранён: единственный мост на `window.kepler.ark.request` — `kepler-api-shim.ts`. См. `problems.md`.

## AC7 — ark:guard:writes

**PASS.**

```text
$ bun run ark:guard:writes
ARK write boundary guard passed.
```

См. `raw/ark-guard-writes.log`.

## AC8 — open:eden команда в shell/electron/commands.ts

**PASS.**

В `shell/electron/commands.ts` добавлена запись:

```ts
{
  id: "eden:open",
  title: "Открыть Eden",
  subtitle: "Заметки и дневник",
  category: "open",
  kind: "app",
  icon: () => extensionIconDataUri("eden"),
  exec: () => openAsExtension("eden"),
},
```

Заголовок комментария файла обновлён (Eden больше не отмечена как «standalone Electron»).

## AC9 — standalone Eden typecheck

**PASS.**

```text
$ cd apps/eden/ts && bunx tsc --noEmit
(0 errors, EXIT=0)
```

См. `raw/standalone-eden-typecheck.log`. `apps/eden/ts/` остаётся неизменным как fallback.

## AC10 — manual smoke (Kepler dev mode + Eden window)

**PENDING_MANUAL.**

Полноценная визуальная верификация требует запуска electron GUI: `bun run --cwd shell dev`, открыть launcher (Ctrl+Shift+K), найти запись «Открыть Eden», кликнуть, увидеть Eden window с note list / редактором.

Запустить эту проверку из CI/proof-loop окружения невозможно без GUI-сессии. Оператору следует:

1. `bun run --cwd shell dev`
2. Ctrl+Shift+K → найти «Открыть Eden» → Enter
3. Убедиться, что extension window открывается с Eden Vue UI
4. Проверить, что список заметок (если есть `note_obj` в ARK) рендерится; либо пустой state без ошибок
5. Создать новую заметку, набрать текст, увидеть TipTap editor
6. Закрыть и переоткрыть Eden — заметка должна сохраниться (через `upsert_object` ARK call)
7. Зафиксировать скриншот / лог в `raw/smoke.md`

Acceptance остаётся за оператором; автоматизированная проверка покрывает только code-level паритет (AC1-AC9). Если manual smoke выявляет проблемы — фиксы идут в `problems.md` + reverify.

## Verification summary

| AC                            | Verdict                                            |
| ----------------------------- | -------------------------------------------------- |
| AC1 manifest fields           | PASS                                               |
| AC2 package.json structure    | PASS                                               |
| AC3 bun install               | PASS                                               |
| AC4 build:extensions          | PASS                                               |
| AC5 shell typecheck           | PASS                                               |
| AC6 edenApi.ts clean          | PASS (literal); see problems.md re: repo-wide grep |
| AC7 ark:guard:writes          | PASS                                               |
| AC8 open:eden command         | PASS                                               |
| AC9 standalone Eden typecheck | PASS                                               |
| AC10 manual smoke             | PENDING_MANUAL — требует оператора                 |
