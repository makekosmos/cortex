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

**Default bump = patch only (`+0.0.1`).** Двигай только третью цифру версии
(`0.1.0 → 0.1.1`, `0.3.11 → 0.3.12`). Minor (`0.1.0 → 0.2.0`) или major
делай только если пользователь явно сказал `minor` / `major` или назвал точную
целевую версию.

**Bump не завершён без build + publish.** После patch edit, docs, commit и push
обязательно собери и опубликуй затронутые targets через существующие скрипты и
`gh` release. Если публикация заблокирована правами/токеном/падающим build'ом,
зафиксируй blocker и не называй bump завершённым.

## Шаги

### 1. Определить target

- Явное имя в команде («бамп eden») → используй его (`eden`, `delphi`, `horologion`, `arrancador`, `shell` = kepler-shell).
- **Без имени** → определи по `git log <last-bump-commit>..HEAD` (последний bump commit обычно `chore: bump ...` или `fix(...) + bump ...`):
  - Все компоненты у которых есть commits, **трогающие соответствующие папки** (`extensions/<id>/`, `shell/`, `packages/visuals/`) → кандидаты на bump.
  - Если у компонента **нет user-visible commits** с последнего bump'а (только docs / typecheck / chore-deps / lint) — пропусти его.
  - Если несколько компонентов → multi-target bump (см. секцию ниже).
- **Несколько имён** («бамп eden delphi») → multi-target bump.
- Если непонятно после диагностики — спроси.

### 1a. Pre-bump checks

Перед любым bump'ом — обязательно:

```powershell
git status --short       # рабочее дерево clean? если нет — закоммить ОТДЕЛЬНО до bump'а
bun run --cwd platform/desktop typecheck
# для extension-bump'а:
bun run --cwd platform/desktop build:extensions
```

Если typecheck / build fail — fix первым, потом bump. Bump чтобы «закрыть test-цикл» / «вытолкнуть фикс» — anti-pattern; bump = release для пользователя.

### 2. Bump patch версии (+0.0.1)

- **Extension** (`extensions/<id>/`):
  - `extensions/<id>/manifest.json` → `"version"`
  - `extensions/<id>/package.json` → `"version"`
  - Оба ОБЯЗАНЫ совпадать. Manifest — источник правды для marketplace / autoupdater, package.json — для workspace. После edit'а:
    ```powershell
    jq -r '.version' extensions/<id>/manifest.json
    jq -r '.version' extensions/<id>/package.json
    # должны вернуть одно и то же
    ```
- **Kosmos desktop shell** (`platform/desktop/`):
  - `platform/desktop/release-versions.json` — источник правды. Ключи `"win"` и `"mac"`, каждый `"MAJOR.MINOR.PATCH"`.
  - `platform/desktop/package.json → "version"` НЕ трогаем — версия инжектируется через `-c.extraMetadata.version` при сборке.
  - Бамп через CLI: `node scripts/release-version.mjs bump --platform <win|mac>` (patch default, `--minor` для minor-parity).
    - Patch: `node scripts/release-version.mjs bump --platform win` (Windows) или `--platform mac` (Mac).
    - Minor parity (оба платформы разом): `node scripts/release-version.mjs bump --minor`.
    - Minor одна платформа: `node scripts/release-version.mjs bump --platform mac --minor`.
  - **Build + publish**: `bun run --cwd platform/desktop build` (Windows) / `bun run --cwd platform/desktop build:mac` (Mac).
    - Скрипт сам читает версию из `release-versions.json`, инжектирует в electron-builder и запускает verify guard.
- **`packages/visuals` / `core/ark/packages/ark`** — НЕ БАМПЯТСЯ. Это workspace internals (`"version": "0.1.0"` зафиксирована), extensions/shell зависят через `workspace:*`. Менять только если будет actual публикация на npm (не сейчас).
- Patch increment (`0.1.2 → 0.1.3`), не minor/major. Minor — только по явной команде пользователя («бамп eden minor»).

### 3. Обновить документацию

**Обязательно**:

- **`STATUS.md`** (корень) — единственный developer-facing changelog проекта. `CHANGELOG.md` отдельный **не существует** — не создавай его.
  - Обнови таблицу версий в шапке (Kepler / Eden / Delphi / Horologion / Arrancador).
  - Добавь раздел с датой bump'а и описанием: `## YYYY-MM-DD — <название итерации> (Kepler X.Y.Z, Eden A.B.C)`.
  - В тоне «развёрнуто, для следующего агента / разработчика» — технические детали, ссылки на коммиты / спеки.
- **`docs-site/whats-new/<area>.md`** — **user-facing** changelog. Тон «о, круто», без жаргона типа `IPC` / `manifest.commands[]`. Добавляй карточку **только если фича user-visible** (то что пользователь заметит): новая команда в launcher'е, новый хоткей, изменение UX, исправление заметного бага. Внутренние рефакторы / fix билдов / typecheck — **не** добавляй.
  - `whats-new/kepler.md` — лаунчер (хоткеи, маркетплейс, focus widget, обновления, settings).
  - `whats-new/extensions.md` — Eden, Horologion, Delphi, Arrancador (per-app section).
  - Формат карточки: `### Заголовок <Badge type="tip" text="0.1.X" />` + 1-3 коротких абзаца на человеческом языке. Указывай что юзер делает, а не как код работает.

**Опционально** (если архитектура / поведение реально изменились):

- `docs-site/apps/<name>.md` — для дев-доки конкретного компонента.
- `docs-site/concepts/<name>-*.md` — для cross-cutting концептов (extension-host, command-bus, distribution).
- `docs-site/agents/forbidden.md` — если новая фича вводит инвариант, нарушение которого опасно (например «не выключай `keepAliveInBackground` для Horologion без переноса side-effects в main»).

**После правок в `docs-site/`**:

- `bun run docs:sync` — регенерация `AGENTS.md` / `CLAUDE.md` / `llms.txt`. Иначе разъедутся.
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
bun run --cwd platform/desktop ext:publish <id>
```

Скрипт сам:

1. Билдит `dist/` через `build:extensions`.
2. Пакует в `<id>-<version>.kext`.
3. Считает SHA-256.
4. `gh release create <id>-v<version> -R yoso-industries/kosmos-extensions ...`.

После публикации — регенерация каталога:

```powershell
bun run --cwd platform/desktop ext:catalog
```

И коммит обновлённого catalog.json в `kosmos-extensions` (если скрипт пишет локально), либо push если он его пушит сам — проверь output `ext:catalog`.

#### Для kepler-shell (лаунчер):

```powershell
# Windows:
bun run --cwd platform/desktop build

# Mac:
bun run --cwd platform/desktop build:mac
```

Версия берётся автоматически из `platform/desktop/release-versions.json` —
убедись, что перед build'ом сделал `bump --platform win` (или `mac`).

::: danger Windows build lock: не расследовать заново
На этой машине установленный/запущенный `KeplerFocusSvc` может держать workspace
artifact `target\release\kepler-focus-svc.exe`. Тогда Cargo/electron build падает
на Windows с:

```text
failed to remove file target\release\kepler-focus-svc.exe
Access is denied. (os error 5)
```

Это **известная Windows/service-lock проблема**, не баг продукта и не повод
тратить время на повторное расследование. Для shell bump/release сразу делай так:

1. Проверь сервис:
   ```powershell
   sc.exe qc KeplerFocusSvc
   ```
   Если `BINARY_PATH_NAME` указывает в `D:\Personal\Hobby\Coding\kosmos\target\release\...`,
   обычный `bun run --cwd platform/desktop build` будет ненадёжен.
2. Собери Rust backend/helper/service в alternate target dir:
   ```powershell
   $env:CARGO_TARGET_DIR = ".tmp\cargo-release"
   cargo build --release --manifest-path Cargo.toml --bin kepler-backend --bin ark-core-rpc --bin kepler-focus-helper --bin kepler-focus-svc
   ```
3. Запускай electron-builder через временный config, где `extraResources` смотрит
   на `.tmp\cargo-release\release\*.exe`, а не на `target\release\*.exe`.
   Не правь постоянный `platform/desktop/package.json` ради этого workaround'а.
4. В evidence/release notes фиксируй как environment workaround. Не называй bump
   завершённым, пока installer/latest.yml реально не собраны и не опубликованы.

Если обычный build уже прошёл без lock — отлично, workaround не нужен. Но при
первом `os error 5` на `target\release\*.exe` не делай серию повторных попыток:
сразу переходи на alternate target dir.
:::

::: warning electron-builder publish step падает без GH_TOKEN
У пользователя нет постоянного `GH_TOKEN` в env. `electron-builder --publish always`
дойдёт до GitHubPublisher и упадёт с `GitHub Personal Access Token is not set`.
**Это OK** — артефакты уже собраны на диск к этому моменту:

```
shell/release/Kepler Setup X.Y.Z.exe
shell/release/Kepler Setup X.Y.Z.exe.blockmap
shell/release/latest.yml
```

Сразу делай **manual GH release через gh CLI** (auth уже настроен через `gh auth login`):

```powershell
cd shell/release
gh release create v<version> -R makekosmos/desktop `
  --title "v<version>" `
  --notes "<краткий changelog>" `
  "Kepler Setup <version>.exe" `
  "Kepler Setup <version>.exe.blockmap" `
  "latest.yml"
```

`latest.yml` **обязательно** прикреплять — autoUpdater старых установок ищет его
для определения новой версии. Без него обновление не подхватится.

После падения electron-builder на `GH_TOKEN` **проверь содержимое `latest.yml`**:

```powershell
Get-Content shell/release/latest.yml
```

Если там осталась старая версия, не загружай его как есть. Создай fresh metadata
для новой версии: файл `latest.yml` должен указывать на реально загружаемый
installer asset, содержать актуальные `version`, `path`, `files[].url`,
`sha512`, `size`, `releaseDate`. В предыдущих релизах updater path использует
hyphen-имя (`Kosmos-Setup-X.Y.Z.exe`), поэтому либо загружай asset с тем же
именем, либо синхронно меняй `path/files[].url` на фактическое имя. Перед
`gh release create` ещё раз проверь `latest.yml` глазами/командой.
:::

### 5a. Multi-target bump в одном запросе

Если bump покрывает несколько компонентов (например после смешанной итерации:
shell + eden + horologion):

- **Версии** правь во всех компонентах одним коммитом: `chore: bump shell 0.2.3→0.2.4, eden 0.1.10→0.1.11, horologion 0.1.5→0.1.6`.
  - Так делают предыдущие multi-bump коммиты в истории (`94bb480b`, `273eb032`).
  - Не разбивай на отдельные `chore(eden)` / `chore(shell)` коммиты — это перетасовка из-за которой `git log --oneline` становится шумным, а сами bump'ы атомарно связаны.
- **STATUS.md / whats-new** — один комбинированный edit + один коммит с docs (можно объединить с bump-коммитом, см. шаблон ниже).
- **Publish** — отдельно per-extension (`ext:publish` принимает один id), потом один `ext:catalog`:
  ```powershell
  bun run --cwd platform/desktop ext:publish eden
  bun run --cwd platform/desktop ext:publish horologion
  # desktop shell — отдельная процедура (build + manual gh release)
  bun run --cwd platform/desktop ext:catalog
  ```
- **Каталог** перегенерируется один раз в конце — захватывает все свежие releases.

### 6. Verify

- `gh release view <id>-v<version> -R yoso-industries/kosmos-extensions` (для extension) или
  `gh release view v<version> -R makekosmos/desktop` (для shell).
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
bun run --cwd platform/desktop ext:publish eden
bun run --cwd platform/desktop ext:catalog
```
