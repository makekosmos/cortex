---
name: bump
description: Поднять patch-версию приложения/extension'а на 0.0.1, обновить документацию, запушить, забилдить и зарелизить через gh CLI. Триггер — пользователь говорит «бамп <name>» или просто «бамп» в контексте конкретного приложения.
allowed-tools: Bash, Read, Edit, Grep, Glob
---

# Бамп

::: danger Бамп — ТОЛЬКО по явному запросу пользователя
Никогда не делай bump версии «попутно» / «логически вместе с фиксом» / «раз изменил manifest, надо бампнуть». Bump = release = пользователи получают update через auto-updater и видят пуш-уведомление, что неприемлемо для каждого мелкого WIP-коммита. Bump делаем **только** когда пользователь напрямую сказал «бамп <name>» или «релизни <name>».

Edit'ить код, чинить баги, обновлять документацию, коммитить и пушить — можно и нужно без bump'а. Bump — отдельная команда пользователя.
:::

Когда пользователь говорит **«бамп»** (опционально с именем: «бамп eden», «бамп shell») — это полный release-flow, не просто edit версии в `package.json`.

## Шаги

### 1. Определить target

- Явное имя в команде → используй его (`eden`, `delphi`, `horologion`, `arrancador`, `shell` = kepler-shell).
- Без имени → смотри последний коммит / уже staged изменения / контекст разговора. Если непонятно — спроси.

### 2. Bump patch версии (+0.0.1)

- **Extension** (`extensions/<id>/`):
  - `extensions/<id>/manifest.json` → `"version"`
  - `extensions/<id>/package.json` → `"version"`
  - Оба должны быть согласованы.
- **kepler-shell** (`shell/`):
  - `shell/package.json` → `"version"`
- Patch increment (`0.1.2 → 0.1.3`), не minor/major.

### 3. Обновить документацию

- `STATUS.md` — если статус приложения изменился (новая фича, фикс заметного бага).
- `CHANGELOG.md` (корень репо) — технический changelog для разработчиков (Keep a Changelog), если фича/фикс существенный.
- **`docs-site/whats-new/<area>.md`** — **user-facing** changelog. Тон «о, круто», без жаргона типа `IPC` / `manifest.commands[]`. Добавляй карточку **только если фича user-visible** (то что пользователь заметит): новая команда в launcher'е, новый хоткей, изменение UX, исправление заметного бага. Внутренние рефакторы / fix билдов / typecheck — **не** добавляй.
  - `whats-new/kepler.md` — лаунчер (хоткеи, маркетплейс, focus, обновления, settings).
  - `whats-new/extensions.md` — Eden, Horologion, Delphi, Arrancador (per-app section).
  - Формат карточки: `### Заголовок <Badge type="tip" text="0.1.X" />` + 1-3 коротких абзаца на человеческом языке. Указывай что юзер делает, а не как код работает.
- `docs-site/apps/<name>.md` или `docs-site/concepts/<name>-*.md` — если поведение/архитектура изменились (это уже для следующих агентов и разработчиков, не для пользователя).
- После правок в `docs-site/` обязательно `bun run docs:sync` — иначе `AGENTS.md` / `CLAUDE.md` разъедутся.
- Если документация уже обновлена в предыдущих коммитах текущей сессии — пропусти этот шаг (не дублируй).

### 4. Commit + push

```powershell
git add <bumped files>
git commit -m "chore(<name>): bump <name> 0.1.X → 0.1.Y"
git push
```

Если bump едет одним коммитом с фиксом/фичей — объединяй в один семантический коммит (`fix(<name>): ... + bump 0.1.X → 0.1.Y`). Если несколько коммитов уже были запушены отдельно — bump отдельным коммитом OK.

### 5. Build + release

#### Для extension (eden, delphi, horologion, arrancador):

```powershell
bun run --cwd shell ext:publish <id>
```

Скрипт сам:

1. Билдит `dist/` через `build:extensions`.
2. Пакует в `<id>-<version>.kext`.
3. Считает SHA-256.
4. `gh release create <id>-v<version> -R yoso-industries/kosmos-extensions ...`.

После публикации — регенерация каталога:

```powershell
bun run --cwd shell ext:catalog
```

И коммит обновлённого catalog.json в `kosmos-extensions` (если скрипт пишет локально), либо push если он его пушит сам — проверь output `ext:catalog`.

#### Для kepler-shell (лаунчер):

```powershell
bun run --cwd shell build
```

`electron-builder --win nsis --publish always` сам бьёт релиз в `yoso-industries/kepler-releases` (см. `shell/package.json → build.publish`).

### 6. Verify

- `gh release view <id>-v<version> -R yoso-industries/kosmos-extensions` (для extension) или
  `gh release view v<version> -R yoso-industries/kepler-releases` (для shell).
- Подтверди что `.kext` / `.exe` + `latest.yml` / `sha256` приложены.

## Запреты (из forbidden.md)

- ❌ Не коммитить `GH_TOKEN` / PAT.
- ❌ Не использовать `git push --force` в main.
- ❌ Не использовать `--no-verify`.
- ❌ Не удалять published GitHub releases retroactive (installed Kepler'ы ломаются).
- ❌ Не менять wire format `catalog.json` без bump `schemaVersion`.
- ❌ Не bundle'ить extensions в Kepler installer (`extraResources`) — lean installer.

## Quick reference (одна строка)

```powershell
# eden 0.1.2 → 0.1.3:
# 1. edit extensions/eden/{manifest.json,package.json}
git add extensions/eden && git commit -m "chore(eden): bump eden 0.1.2 → 0.1.3" && git push
bun run --cwd shell ext:publish eden
bun run --cwd shell ext:catalog
```
