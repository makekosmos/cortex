# Архитектурный рефакторинг Kosmos монорепо

**Дата:** 2026-05-14
**Тип:** proposal-only (миграция отдельной задачей)
**Контекст brand-swap:** ecosystem = **Kosmos**, launcher = **Kepler**
**Связанные документы:** `docs-site/concepts/architecture.md`, `docs-site/apps/kepler-roadmap.md`, `STATUS.md`, `.agent/tasks/2026-05-14-extension-installer-mvp/spec.md`

---

## TL;DR

Текущая структура `apps/` смешивает четыре несовместимых концепта (legacy standalone Electron-апки, retired Rust launcher, новый Electron host, мигрированные Vue extensions внутри host'а). Это и есть тот «бардак», на который жалуется пользователь.

Документ предлагает **три варианта** с разным радиусом изменений. Рекомендованный — **Variant 2** (см. секцию «Рекомендация» в конце). Variant 1 — fallback если на refactor нет времени; Variant 3 — overkill для post-pivot периода, когда продукт ещё не стабилен.

Документ также описывает **post-refactor design** для трёх ближайших задач: persistent extension user data, Dashboard rework, поглощение usage-tracker в kepler-backend.

---

## 1. Цели и принципы

### 1.1 Что значит «чёткая архитектура» в этом репо

Пользователь сформулировал ощущение, а не требование. Раскрываем в принципы:

1. **Top-level директория = роль, а не «куча всего что есть»**. Если кто-то видит `apps/`, он должен понимать одно ясное предложение: «здесь живут продуктовые apps». Сейчас `apps/` — это «здесь живут разные штуки: апки, не-апки, и extensions, которые не апки, но похожи».

2. **One place of truth для каждого деплоябля**. Сейчас Dashboard живёт в `apps/dashboard/` И в `apps/kepler-shell/extensions/dashboard/`. Это создаёт когнитивную нагрузку: «где правда»? Правда — в extension'е, но старая папка не удалена. То же — Horologion, Delphi, Arrancador.

3. **Legacy явно отделён**. `apps/kepler/` — мёртвый Rust launcher (Phase 8 retire), но лежит как живой. Должен быть либо удалён, либо явно помечен как legacy/archive.

4. **Имена директорий описывают роль, а не происхождение**. `services/kepler-watcher/` — что это? Это watchdog (3 строки Cargo.toml, нетронут). Имя нормальное. А вот `apps/ark-service/` — это Android Kotlin модуль, который имеет очень мало общего с другими `apps/`.

5. **Workspace conventions согласованы с layout'ом**. Сейчас `package.json` имеет хак `apps/delphi/ts`, `apps/eden/ts` — потому что Eden и Delphi имеют под-workspace внутри. Это не плохо, но это symptom of layout'а, который пытается одной формой описать слишком разные вещи.

### 1.2 Что **не** считается целью

- Переименование packages `@kosmos/ark` → что-то ещё. Брэнд только что swap'нут — двойная миграция = двойной риск.
- Переименование самого monorepo / git remote.
- Миграция Eden в extension в рамках этого refactor'а (Phase 6 отдельно).
- «Чистка ради чистки» — каждый шаг должен иметь обоснование, либо в терминах «убирает дубликат», либо «убирает путаницу при чтении», либо «разблокирует следующую задачу».

### 1.3 Источники инвариантов

- **Junction'ы bun на Windows**: `CLAUDE.md` секция «Файловые операции на Windows» — `Move-Item -Force` на `apps/<name>/` с присутствующим `node_modules/` разрешает junction в `packages/*` и удаляет таргет. Пользователь уже терял часы untracked-работы. Это **жёсткое ограничение** всех migration-шагов ниже.
- **Brand-swap freshness**: 836 файлов swap'нуто 2026-05-14. Любое имя, содержащее `Kosmos` или `Kepler` — критическое, не переименовывать ad-hoc.
- **Phase 8 retire**: `apps/kepler/` обозначен к удалению — у refactor'а есть лёгкое legitimate-окно его убрать.
- **electron-builder extraResources**: paths `../../services/kepler-backend/target/release/kepler-backend.exe`, `../../packages/ark-core/rust/target/release/ark-core-rpc.exe`, `extensions` — относительно `apps/kepler-shell/package.json`. Любое перемещение shell'а ломает это.

---

## 2. Текущий layout (baseline)

```
kepler/
├── apps/
│   ├── ark-service/         Android Kotlin Room provider (Delphi backend)
│   ├── arrancador/          legacy standalone Electron (DUP с extensions/arrancador)
│   ├── dashboard/           legacy standalone Electron (DUP, по сути уже пустой)
│   ├── delphi/              {ts/, kotlin/} — TS = legacy standalone (DUP), kotlin = Android
│   ├── eden/                {ts/} — живой standalone Electron (Phase 6 не сделан)
│   ├── horologion/          legacy standalone Electron (DUP)
│   ├── kepler/              RETIRED Rust gpui launcher (Phase 8 — удалить)
│   └── kepler-shell/        АКТИВНЫЙ Electron host + extensions/ внутри
│       ├── electron/
│       ├── extensions/
│       │   ├── arrancador/  ◀ ЖИВАЯ копия
│       │   ├── dashboard/   ◀ ЖИВАЯ копия
│       │   ├── delphi/      ◀ ЖИВАЯ копия
│       │   └── horologion/  ◀ ЖИВАЯ копия
│       └── src/
├── packages/
│   ├── ark-core/            Rust ARK runtime + ark-core-rpc binary
│   ├── kosmos-ark/          TS SDK (ЖИВОЙ)
│   ├── kosmos-visuals/      UI components (ЖИВОЙ)
│   ├── kepler-ark/          МУСОР (только node_modules от старого workspace)
│   └── kepler-visuals/      МУСОР (то же)
├── services/
│   ├── ark-relay-server/    Rust relay (WS NAT-обход)
│   ├── kepler-backend/      ЖИВОЙ Rust backend (lib + bin)
│   ├── kepler-watcher/      tiny watchdog (3 строки deps)
│   └── usage-tracker/       standalone Rust process (10 файлов, 2780 LoC)
├── packages/, docs-site/, scripts/, docs/, .agent/, ...
└── package.json (bun workspaces: apps/*, apps/delphi/ts, apps/eden/ts, services/*, packages/*, docs-site)
```

::: warning Проблемы baseline'а

- `apps/{dashboard,delphi/ts,horologion,arrancador}/` — мёртвые дубликаты живых extension'ов.
- `apps/kepler/` — retired Rust binary, нет смысла держать.
- `packages/{kepler-ark,kepler-visuals}/` — оставлены junction-target'ы после brand swap.
- Extensions вложены **внутри** `apps/kepler-shell/extensions/`, хотя концептуально они = продуктовые apps, а shell = их host.
- `apps/ark-service/` (Android) лежит рядом с Electron apps, никакой связи кроме слова «service» в имени.
- `services/usage-tracker/` — standalone, хотя по сути должен быть **модулем** kepler-backend (один процесс, один ARK).
- `services/kepler-watcher/` — почти пустой, оставлен на всякий случай.
- Workspace glob `apps/*` + два явных `apps/eden/ts`, `apps/delphi/ts` — leakage внутренней структуры в root config.
  :::

---

## 3. Variant 1 — Минимальный refactor

### 3.1 Идея

Не двигать большие папки. Только:

1. Удалить мёртвые дубликаты в `apps/`.
2. Удалить `packages/kepler-{ark,visuals}/` мусор.
3. Удалить `apps/kepler/` (Phase 8 retire).
4. Документировать что `apps/kepler-shell/extensions/` — это **product apps** (концептуально), не часть shell internals.

### 3.2 Final layout

```
kepler/
├── apps/
│   ├── ark-service/         Android Kotlin (унаследовано)
│   ├── eden/ts/             standalone Electron (до Phase 6)
│   └── kepler-shell/        Electron host
│       ├── electron/
│       ├── extensions/
│       │   ├── arrancador/
│       │   ├── dashboard/
│       │   ├── delphi/
│       │   └── horologion/
│       └── src/
├── packages/
│   ├── ark-core/
│   ├── kosmos-ark/
│   └── kosmos-visuals/
├── services/
│   ├── ark-relay-server/
│   ├── kepler-backend/
│   ├── kepler-watcher/
│   └── usage-tracker/
├── docs-site/, scripts/, docs/, .agent/
└── package.json
```

### 3.3 Миграционные шаги

::: danger Junction safety — обязательная prelude перед любым шагом
Перед `git mv` или `Remove-Item` папок, которые могут содержать junction'ы внутри `node_modules/@kosmos/*`:

```powershell
Remove-Item -Recurse -Force apps\dashboard\node_modules -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force apps\horologion\node_modules -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force apps\arrancador\node_modules -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force apps\delphi\ts\node_modules -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force apps\kepler\target -ErrorAction SilentlyContinue
```

Только **после** удаления `node_modules` можно делать `git rm -rf`. Параллельно `git status` должен быть чистым в `packages/*` (никаких `??` файлов), иначе junction разрешится и удалит untracked в target'е.
:::

Шаги (по порядку):

1. **Pre-flight check**:

   ```powershell
   git status --short              # должен быть чистый
   git status --short packages/    # ОБЯЗАТЕЛЬНО чисто
   bun run ark:smoke               # baseline зелёный
   ```

2. **Cleanup node_modules перед удалением** (см. callout выше).

3. **Удалить дубликаты apps**:

   ```powershell
   git rm -rf apps/dashboard apps/horologion apps/arrancador
   git rm -rf apps/delphi/ts        # оставить apps/delphi/kotlin (Android)
   ```

   Заметка: `apps/delphi/` остаётся как контейнер для `kotlin/`. Альтернативно перенести `apps/delphi/kotlin` → `apps/delphi-android` и удалить весь `apps/delphi/`. См. шаг 5.

4. **Удалить retired Rust launcher**:

   ```powershell
   git rm -rf apps/kepler
   ```

5. **Перенести Android модуль** (опционально, но рекомендуется чтобы убрать пустой `apps/delphi/`):

   ```powershell
   git mv apps/delphi/kotlin apps/delphi-android
   git rm -rf apps/delphi          # уже пустой
   ```

6. **Удалить junction-target мусор**:

   ```powershell
   git rm -rf packages/kepler-ark packages/kepler-visuals
   ```

   Перед этим проверить что внутри нет ничего кроме `node_modules/` и `dist/`:

   ```powershell
   Get-ChildItem packages\kepler-ark -Force
   Get-ChildItem packages\kepler-visuals -Force
   ```

7. **Обновить root `package.json` workspaces**:

   ```diff
   "workspaces": [
     "apps/*",
   - "apps/delphi/ts",
     "apps/eden/ts",
     "services/*",
     "packages/*",
     "docs-site"
   ]
   ```

   Если применён шаг 5 — `apps/eden/ts` тоже можно перенести в `apps/eden-desktop` чтобы убрать второй workspace exception. Это оптяно (см. Variant 2).

8. **Обновить `scripts/sync-agents-docs.mjs`**:

   ```diff
   - { dest: "apps/eden/AGENTS.md", src: "apps/eden.md", title: "Eden" },
   - { dest: "apps/delphi/AGENTS.md", src: "apps/delphi.md", title: "Delphi" },
   - { dest: "apps/arrancador/AGENTS.md", src: "apps/arrancador.md", title: "Arrancador" },
   - { dest: "apps/dashboard/AGENTS.md", src: "apps/dashboard.md", title: "Dashboard" },
   - { dest: "apps/horologion/AGENTS.md", src: "apps/horologion.md", title: "Horologion" },
   - { dest: "apps/eden/ts/AGENTS.md", src: "apps/eden.md", title: "Eden — TS workspace" },
   - { dest: "apps/delphi/kotlin/AGENTS.md", src: "apps/delphi.md", title: "Delphi — Kotlin workspace" },
   + { dest: "apps/eden/AGENTS.md", src: "apps/eden.md", title: "Eden" },
   + { dest: "apps/eden/ts/AGENTS.md", src: "apps/eden.md", title: "Eden — TS workspace" },
   + { dest: "apps/delphi-android/AGENTS.md", src: "apps/delphi.md", title: "Delphi — Android" },
   + { dest: "apps/kepler-shell/extensions/dashboard/AGENTS.md", src: "apps/dashboard.md", title: "Dashboard" },
   + { dest: "apps/kepler-shell/extensions/horologion/AGENTS.md", src: "apps/horologion.md", title: "Horologion" },
   + { dest: "apps/kepler-shell/extensions/delphi/AGENTS.md", src: "apps/delphi.md", title: "Delphi" },
   + { dest: "apps/kepler-shell/extensions/arrancador/AGENTS.md", src: "apps/arrancador.md", title: "Arrancador" },
   ```

9. **Обновить документацию** (только текст, не пути deploy'я):
   - `docs-site/concepts/architecture.md` — карта apps, убрать ссылки на дубликаты.
   - `STATUS.md` — Phase 8 «retire apps/kepler/» → ✅.
   - `docs-site/apps/kepler-roadmap.md` — то же.

10. **Прогнать гварды**:
    ```powershell
    bun install                   # пересоздать junction'ы
    bun run ark:guard:writes
    bun run ark:smoke
    bun run --cwd apps/kepler-shell typecheck
    bun run --cwd apps/kepler-shell build:extensions
    ```

### 3.4 Что **не** ломается

- `vite.config.mjs` / `vite.extensions.config.mjs` пути — `../../packages/kosmos-ark/...` — без изменений.
- `apps/kepler-shell/package.json` build section с extraResources — без изменений (paths остаются `../../services/kepler-backend/...`).
- `services/kepler-backend/Cargo.toml`, `services/usage-tracker/Cargo.toml` deps — без изменений (`ark-core = { path = "../../packages/ark-core/rust" }` живой).

### 3.5 Risks

::: danger Risk 1 — Junction discharge
Если запустить `git rm` или `Remove-Item -Recurse -Force` на `apps/dashboard/` пока `node_modules/@kosmos/*` живой junction в `packages/kosmos-*/` — он будет разрешён и удалит **target**. Mitigation: всегда удалять `node_modules` first, контрольный `git status packages/` после.
:::

::: warning Risk 2 — Untracked work
В `packages/kosmos-visuals/` может лежать untracked WIP (пользователь уже терял). Mitigation: `git stash --include-untracked` перед операциями, или явный `git status packages/` review.
:::

::: warning Risk 3 — `bun.lock` invalidation
Удаление workspace членов меняет lockfile graph. После `bun install` коммитить новый `bun.lock`.
:::

### 3.6 Effort / Reversibility

- **Effort**: 2–4 часа (включая verify, ark:smoke, manual launcher start, build:extensions).
- **Reversibility**: высокая. Все удаления через `git rm` — `git revert` восстанавливает. Junction-driven losses возможны только если нарушен safety protocol.

---

## 4. Variant 2 — Средний refactor (рекомендован)

### 4.1 Идея

Variant 1 + переименование top-level так, чтобы layout отражал текущие роли. Главные изменения:

- `apps/kepler-shell/` → `shell/` на top-level.
- `apps/kepler-shell/extensions/` → `extensions/` на top-level.
- `apps/eden/ts/` → `apps/eden/` (или `apps/eden-desktop/`) — убрать вложенный workspace.
- `apps/ark-service/` + `apps/delphi/kotlin` → `mobile/` top-level (Android модули отделены явно).
- `services/` остаётся для Rust-backend сервисов.
- Новый `legacy/` top-level для retired-but-not-deleted кода (опционально — на момент migration `apps/kepler/` уже удалён в Variant 1; `legacy/` нужен только если хочется сохранить).

### 4.2 Final layout

```
kepler/
├── shell/                       Electron host (Kepler launcher)
│   ├── electron/
│   ├── src/
│   ├── scripts/                 install-extension.mjs, uninstall-extension.mjs, dev-extensions.mjs
│   ├── build/                   afterPack hook, icons
│   ├── package.json             { "name": "kepler-shell", ... }
│   ├── vite.config.mjs
│   └── vite.extensions.config.mjs
├── extensions/                  Vue extension bundles (== продуктовые apps)
│   ├── dashboard/
│   ├── horologion/
│   ├── delphi/
│   └── arrancador/
├── apps/                        Standalone desktop apps (не extensions)
│   └── eden/                    Vue + Electron + Heart sidecar (до Phase 6 migration)
├── mobile/                      Android модули
│   ├── delphi/                  Kotlin + Room (бывший apps/delphi/kotlin)
│   └── ark-service/             ContentProvider (бывший apps/ark-service)
├── services/                    Rust services (headless процессы)
│   ├── kepler-backend/          + интегрированный usage_tracker модуль (см. секция 8)
│   ├── ark-relay-server/
│   └── kepler-watcher/
├── packages/                    Shared libs (TS SDK, Rust crate, UI lib)
│   ├── ark-core/                Rust crate + ark-core-rpc binary
│   ├── kosmos-ark/              TS SDK
│   └── kosmos-visuals/          Vue components + CSS tokens
├── docs-site/                   VitePress
├── scripts/                     repo-wide helpers
├── docs/                        ADR-style docs
├── .agent/                      proof-loop tasks, proposals
├── package.json
└── ...
```

### 4.3 Конкретные перемещения

```
apps/kepler-shell/                    →  shell/
apps/kepler-shell/extensions/<id>/    →  extensions/<id>/   (для всех 4)
apps/eden/ts/                         →  apps/eden/          (промоут одного уровня)
apps/eden/  (parent контейнер с README) -- merge с ts/ (см. шаги ниже)
apps/delphi/kotlin/                   →  mobile/delphi/
apps/ark-service/                     →  mobile/ark-service/
apps/kepler/                          →  УДАЛИТЬ (как в Variant 1)
apps/dashboard/                       →  УДАЛИТЬ (дубликат)
apps/horologion/                      →  УДАЛИТЬ (дубликат)
apps/arrancador/                      →  УДАЛИТЬ (дубликат)
apps/delphi/ts/                       →  УДАЛИТЬ (дубликат)
apps/delphi/  (опустевший parent)     →  УДАЛИТЬ
packages/kepler-ark/                  →  УДАЛИТЬ (мусор)
packages/kepler-visuals/              →  УДАЛИТЬ (мусор)
```

### 4.4 Миграционные шаги (детально)

::: danger Junction safety
Каждый `git mv` / `Remove-Item` на папку с `node_modules/` **ОБЯЗАТЕЛЬНО** prefix'нуть удалением `node_modules`. Полный список папок для очистки:

```powershell
$dirs = @(
  'apps\kepler-shell\node_modules',
  'apps\kepler-shell\extensions\dashboard\node_modules',
  'apps\kepler-shell\extensions\horologion\node_modules',
  'apps\kepler-shell\extensions\delphi\node_modules',
  'apps\kepler-shell\extensions\arrancador\node_modules',
  'apps\dashboard\node_modules',
  'apps\horologion\node_modules',
  'apps\arrancador\node_modules',
  'apps\delphi\ts\node_modules',
  'apps\eden\ts\node_modules',
  'apps\ark-service\node_modules',
  'apps\kepler\target'
)
foreach ($d in $dirs) { if (Test-Path $d) { Remove-Item -Recurse -Force $d } }
```

**Перед** запуском: `git status packages/` ДОЛЖЕН быть чистым (никаких `??`). Иначе stash и затем повторить.
:::

Шаги:

1. **Pre-flight**:

   ```powershell
   git status --short
   git status --short packages/
   git stash --include-untracked   # если есть что прятать
   bun run ark:smoke
   ```

2. **Cleanup node_modules** (см. callout).

3. **Сначала удалить дубликаты** (как Variant 1, шаги 3-6):

   ```powershell
   git rm -rf apps/dashboard apps/horologion apps/arrancador apps/delphi/ts apps/kepler
   git rm -rf packages/kepler-ark packages/kepler-visuals
   ```

4. **Промоут Eden TS** (избавиться от лишней вложенности):

   ```powershell
   # apps/eden/ts/* → apps/eden/* — но apps/eden/ уже занят (AGENTS.md, README.md).
   # Стратегия: переименовать parent в apps/eden-desktop, потом подняться.
   git mv apps/eden/ts apps/eden-desktop-tmp
   git rm apps/eden/AGENTS.md apps/eden/README.md   # eden/ опустеет
   # apps/eden теперь пустой — удалим
   Remove-Item apps/eden -Force
   git mv apps/eden-desktop-tmp apps/eden
   ```

   Альтернативно: оставить `apps/eden/ts/` и не трогать до Phase 6 (см. Risks).

5. **Mobile модули**:

   ```powershell
   New-Item -ItemType Directory mobile
   git mv apps/delphi/kotlin mobile/delphi
   git rm -rf apps/delphi               # пустой parent
   git mv apps/ark-service mobile/ark-service
   ```

6. **Промоут shell**:

   ```powershell
   git mv apps/kepler-shell shell-tmp
   # extensions поднять до того как shell станет shell/
   git mv shell-tmp/extensions extensions
   git mv shell-tmp shell
   ```

   После этого `shell/` содержит electron/, src/, scripts/, build/, package.json, vite configs. `extensions/` — top-level с 4 поддиректориями.

7. **`apps/` теперь почти пустой**. Если внутри только `eden/` — оставить. Если нужен `apps/README.md` — обновить.

8. **Обновить `shell/package.json`**:

   ```diff
   "scripts": {
   -  "build:backend": "cargo build --release --manifest-path ../../services/kepler-backend/Cargo.toml ...",
   +  "build:backend": "cargo build --release --manifest-path ../services/kepler-backend/Cargo.toml ...",
   -  "build:backend:dev": "cargo build --manifest-path ../../services/kepler-backend/Cargo.toml ...",
   +  "build:backend:dev": "cargo build --manifest-path ../services/kepler-backend/Cargo.toml ...",
   }
   "build": {
     "extraResources": [
   -    { "from": "../../services/kepler-backend/target/release/kepler-backend.exe", "to": "..." },
   +    { "from": "../services/kepler-backend/target/release/kepler-backend.exe", "to": "..." },
   -    { "from": "../../packages/ark-core/rust/target/release/ark-core-rpc.exe", "to": "..." },
   +    { "from": "../packages/ark-core/rust/target/release/ark-core-rpc.exe", "to": "..." },
   -    { "from": "extensions", "to": "extensions", ... },
   +    { "from": "../extensions", "to": "extensions", ... },
     ]
   }
   ```

9. **Обновить `shell/vite.config.mjs` и `shell/vite.extensions.config.mjs`**:

   ```diff
   - extensionsRoot = path.resolve(__dirname, "extensions")
   + extensionsRoot = path.resolve(__dirname, "../extensions")
   ...
   - "@kosmos/ark": path.resolve(__dirname, "../../packages/kosmos-ark/src/index.ts"),
   + "@kosmos/ark": path.resolve(__dirname, "../packages/kosmos-ark/src/index.ts"),
   - "@kosmos/visuals/theme/css": path.resolve(__dirname, "../../packages/kosmos-visuals/theme/css-variables.css"),
   + "@kosmos/visuals/theme/css": path.resolve(__dirname, "../packages/kosmos-visuals/theme/css-variables.css"),
   - "@kosmos/visuals": path.resolve(__dirname, "../../packages/kosmos-visuals"),
   + "@kosmos/visuals": path.resolve(__dirname, "../packages/kosmos-visuals"),
   ```

10. **Обновить `shell/scripts/install-extension.mjs`, `shell/scripts/uninstall-extension.mjs`, `shell/scripts/dev-extensions.mjs`**:
    - Любые `path.resolve(__dirname, "../extensions/...")` или `"../../extensions/..."` пересчитать с учётом нового `shell/` location.
    - `dev-extensions.mjs` orchestrator проходит по `extensions/<id>/vite.config.mjs` — теперь это `../extensions/<id>/vite.config.mjs`.

11. **Обновить `extensions/<id>/vite.config.mjs`** (для каждого):
    - Aliasи `@kosmos/ark` и `@kosmos/visuals` — relative path был `../../../packages/...` (extension сидел в `apps/kepler-shell/extensions/<id>/`), теперь стал `../../packages/...` (extension в `extensions/<id>/`).

12. **Обновить root `package.json` workspaces**:

    ```diff
    "workspaces": [
    -  "apps/*",
    -  "apps/delphi/ts",
    -  "apps/eden/ts",
    -  "services/*",
    -  "packages/*",
    -  "docs-site"
    +  "apps/*",
    +  "shell",
    +  "extensions/*",
    +  "services/*",
    +  "packages/*",
    +  "mobile/*",
    +  "docs-site"
    ]
    ```

    Заметка: `mobile/*` — Gradle проекты, у них нет `package.json`. Bun их проигнорирует, но мы оставляем glob для документационной симметрии. Альтернативно — не включать `mobile/*` в bun workspaces, оно нужно только для документации.

13. **Обновить `scripts/sync-agents-docs.mjs`** — все per-app AGENTS.md destinations:

    ```diff
    - { dest: "apps/eden/AGENTS.md", ... }
    + { dest: "apps/eden/AGENTS.md", ... }     // не меняется если выбрана опция оставить eden/
    - { dest: "apps/eden/ts/AGENTS.md", ... }
    + // удалить (eden/ts/ больше нет, src/ Eden теперь прямо в apps/eden/)
    - { dest: "apps/delphi/AGENTS.md", ... }
    - { dest: "apps/delphi/kotlin/AGENTS.md", ... }
    + { dest: "mobile/delphi/AGENTS.md", src: "apps/delphi.md", title: "Delphi — Android" }
    - { dest: "apps/dashboard/AGENTS.md", ... }
    + { dest: "extensions/dashboard/AGENTS.md", src: "apps/dashboard.md", title: "Dashboard" }
    // аналогично для horologion / delphi / arrancador
    + { dest: "shell/AGENTS.md", src: "apps/kepler.md", title: "Kepler shell" }
    ```

14. **Обновить `services/usage-tracker/package.json`** (если есть scripts ссылающиеся на относительные пути) и `Cargo.toml` paths (если бы они ссылались на `apps/...` — но они ссылаются только на `../../packages/ark-core/rust`, что не меняется).

15. **`bun install`** — пересоздать junction'ы под новые workspace globs.

16. **Обновить документацию** (текстовое):
    - `docs-site/concepts/architecture.md` — обновить диаграмму, карту компонентов.
    - `docs-site/apps/kepler-roadmap.md` — Phase 8 retire + новая layout-таблица.
    - `STATUS.md` — обновить путь карту.
    - `CLAUDE.md` и `AGENTS.md` regen через `bun run docs:sync`.

17. **Verify pipeline**:
    ```powershell
    bun install
    bun run docs:sync
    bun run ark:guard:writes
    bun run ark:smoke
    bun run --cwd shell typecheck
    bun run --cwd shell build:backend:dev
    bun run --cwd shell build:extensions
    bun run --cwd shell dev          # smoke launcher
    cargo build --manifest-path services/kepler-backend/Cargo.toml --lib
    cargo test --manifest-path services/kepler-backend/Cargo.toml --lib
    ```

### 4.5 Что ломается / адаптировать

- **vite configs** во всех 4 extension'ах + 1 shell — `../../packages/` → `../packages/`. Это сейчас mechanical и проверяемо. См. шаги 9 и 11.
- **electron-builder paths**: extraResources `from`. См. шаг 8.
- **`shell/scripts/*.mjs`** — любой `path.resolve(__dirname, ...)` с шагом `..` пересчитать.
- **CI** (если есть GitHub Actions / hooks ссылающиеся на `apps/kepler-shell/...`) — `lefthook.yml` проверить:
  ```powershell
  Get-Content lefthook.yml | Select-String "apps|services|packages"
  ```
- **`scripts/check-ark-write-boundaries.mjs`** — если он сканит `apps/*/electron/main/services/`, нужно расширить glob на `extensions/*/electron/...` (хотя extensions не имеют electron main).
- **`docs-site/.vitepress/config.ts`** — если есть sidebar links на `/apps/dashboard.md` и т.п., убедиться что они корректны (это VitePress route, не file path).
- **AGENTS.md / CLAUDE.md** regen — `sync-agents-docs.mjs` нужно обновить mapping (шаг 13).
- **Playwright configs** — `apps/kepler-shell/playwright.config.ts` → `shell/playwright.config.ts` + любые fixture paths внутри.

### 4.6 Risks

::: danger Risk 1 — Junction discharge при `git mv` shell'а
`apps/kepler-shell/node_modules/` содержит junction'ы к `packages/kosmos-ark`, `packages/kosmos-visuals`. `git mv` сам по себе не разрешает junction, но Windows file APIs которые он использует — могут. Mitigation: **сначала** удалить `apps/kepler-shell/node_modules`, **потом** `git mv`. Описано в callout выше.
:::

::: danger Risk 2 — Path-relative aliasы в Vue файлах
В `.vue` файлах внутри extensions могут быть imports типа `import { Foo } from "../../../packages/kosmos-visuals/..."` (хотя по умолчанию все через alias `@kosmos/visuals`). Mitigation: `grep -r "kosmos-visuals\|kosmos-ark\|kepler-backend\|services/" extensions/` после миграции — должны быть только alias-import'ы, ни одного relative.
:::

::: warning Risk 3 — Workspace name conflicts
`shell/package.json` name = `"kepler-shell"`. После переезда `shell/` он остаётся под тем же именем, но bun workspace member-id меняется с `apps/kepler-shell` на `shell`. `bun.lock` нужно перегенерировать.
:::

::: warning Risk 4 — Long manual verification
Этот Variant требует ручного запуска Kepler (`shell/dev`), check'а что все 4 extensions открываются, что launcher hotkey работает, что backend стартует, что tray menu появляется. Без manual verification typecheck/build могут зелёные при сломанном runtime.
:::

::: warning Risk 5 — Phase 6 Eden migration coordination
Если Eden Phase 6 миграция в extension запланирована скоро, может быть проще не двигать `apps/eden/ts/` сейчас, а сделать одним движением: `apps/eden/ts/` → `extensions/eden/` в рамках Phase 6. Mitigation: оставить `apps/eden/ts/` нетронутым в этом refactor'е (workspace glob прежний для него), переименовать только в Phase 6.
:::

### 4.7 Effort / Reversibility

- **Effort**: 1–2 рабочих дня. ~6–8 часов чистой работы + ~3 часа verify (typecheck, build, manual smoke, e2e если есть).
- **Reversibility**: средняя. `git revert` теоретически работает, но если уже сделан `bun install` после migration — `bun.lock` нужно тоже revert'нуть. Все `git mv` обратимы, junction-discharge — нет.

---

## 5. Variant 3 — Радикальный refactor

### 5.1 Идея

Variant 2 + дополнительно:

- Workspace Cargo (опционально) — собрать все Rust crates в один `Cargo.toml` workspace в корне. Преимущества: общая target/ дир (faster cold build), unified deps version management. Недостатки: deps между не-зависимыми crate'ами становятся implicit.
- Слияние usage-tracker → kepler-backend как модуль (см. секция 8). В Variant 3 это **делается одновременно** с layout-refactor'ом (один большой PR). В Variant 1/2 — отдельная задача после layout-refactor'а.
- Переход на `extensions/` everywhere — Eden тоже перемещается в `extensions/eden/` (форсит Phase 6 migration внутрь refactor'а).
- Reorganization `packages/` — `ark-core` (Rust) выделить в `crates/ark-core/`, TS-пакеты остаются в `packages/`. Это разделяет mental model «Rust libs» vs «TS libs».

### 5.2 Final layout

```
kepler/
├── shell/
├── extensions/
│   ├── dashboard/
│   ├── horologion/
│   ├── delphi/
│   ├── arrancador/
│   └── eden/                    ◀ ПЕРЕНЕСЁН (Phase 6)
├── services/
│   ├── kepler-backend/          + usage_tracker модуль внутри src/
│   ├── ark-relay-server/
│   └── kepler-watcher/
├── crates/                      ◀ Rust libs/crates отделены
│   └── ark-core/
├── packages/                    ◀ только TS packages
│   ├── kosmos-ark/
│   └── kosmos-visuals/
├── mobile/
│   ├── delphi/
│   └── ark-service/
├── legacy/                      ◀ замороженный код для отката
│   └── usage-tracker/           ◀ standalone версия (на случай если merge не зайдёт)
├── docs-site/, scripts/, docs/, .agent/
├── Cargo.toml                   ◀ workspace root (members: services/*, crates/*)
└── package.json
```

### 5.3 Конкретные изменения относительно Variant 2

- `packages/ark-core/` → `crates/ark-core/`. Все `path = "../../packages/ark-core/rust"` в `Cargo.toml`'ах меняются на `path = "../../crates/ark-core/rust"` (или становятся implicit через workspace inheritance).
- Создаётся корневой `Cargo.toml` с workspace members.
- `services/usage-tracker/` → `legacy/usage-tracker/` (заморозка) **после** того как модуль `usage_tracker.rs` встроен в `services/kepler-backend/src/`.
- `apps/eden/` → `extensions/eden/` — требует адаптации Eden preload API (Phase 6 work).

### 5.4 Cargo workspace setup

Новый корневой `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/ark-core/rust",
    "services/kepler-backend",
    "services/ark-relay-server",
    "services/kepler-watcher",
]
exclude = [
    "apps/arrancador/sidecar",   # если ещё живой
    "legacy/usage-tracker",
]

[workspace.dependencies]
ark-core = { path = "crates/ark-core/rust" }
rusqlite = { version = "0.32", features = ["bundled"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net", "time", "sync", "io-util", "io-std", "process", "signal"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = "0.4"
thiserror = "1"
uuid = { version = "1", features = ["v4"] }
```

Per-crate `Cargo.toml`'ы становятся короче:

```toml
[package]
name = "kepler-backend"
version = "0.1.0"
edition = "2021"

[dependencies]
ark-core = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
rusqlite = { workspace = true }
```

### 5.5 Дополнительные миграционные шаги (поверх Variant 2)

1. **Workspace Cargo**:
   - Создать корневой `Cargo.toml`.
   - В каждом per-crate `Cargo.toml` заменить explicit deps на `{ workspace = true }`.
   - **Внимание**: `target/` директории. Сейчас каждый crate имеет свою `target/`. После workspace они переедут в `<root>/target/`. Очистить per-crate target'ы.
   - Обновить `shell/package.json` `build:backend`:
     ```diff
     - "build:backend": "cargo build --release --manifest-path ../services/kepler-backend/Cargo.toml --bin kepler-backend && cargo build --release --manifest-path ../packages/ark-core/rust/Cargo.toml --bin ark-core-rpc",
     + "build:backend": "cargo build --release --workspace --bin kepler-backend --bin ark-core-rpc",
     ```
   - Обновить electron-builder extraResources:
     ```diff
     - { "from": "../services/kepler-backend/target/release/kepler-backend.exe", ... }
     - { "from": "../packages/ark-core/rust/target/release/ark-core-rpc.exe", ... }
     + { "from": "../target/release/kepler-backend.exe", ... }
     + { "from": "../target/release/ark-core-rpc.exe", ... }
     ```

2. **`packages/ark-core/` → `crates/ark-core/`**:

   ```powershell
   git mv packages/ark-core crates/ark-core
   ```

   Обновить все `Cargo.toml`'ы с `path = "../../packages/ark-core/rust"` → `"../../crates/ark-core/rust"` (или сразу через workspace inheritance).

3. **Eden → extension** (Phase 6 work, не самоочевидно):
   - TipTap editor, Heart Rust sidecar, vault store hardening — всё это нужно адаптировать к extension API.
   - Без дополнительных недель работы Eden в extension'е работать **не будет**.

4. **usage-tracker → kepler-backend модуль**:
   - Подробно — секция 8 ниже.
   - В Variant 3 — делается в том же PR.

5. **`legacy/`**:
   - `git mv services/usage-tracker legacy/usage-tracker` после слияния и verify.
   - `legacy/README.md` объясняет что код заморожен, реактивация требует proof loop.

### 5.6 Risks (дополнительно к Variant 2)

::: danger Risk 1 — Cargo workspace target migration
Сейчас каждая Rust crate собирается в свою `target/`. После переезда в workspace `target/` становится shared. Если запустить `cargo build` **до** удаления per-crate `target/` — ничего не сломается, но мусор останется. Также часть deps будет пересобрана с нуля. **Cold build time**: kepler-backend release ~3-5 min, ark-core ~2 min — после workspace это всё пересоберётся.
:::

::: danger Risk 2 — Phase 6 Eden scope creep
Eden migration в extension — задача на 2 недели (по `STATUS.md`). Втягивать её в архитектурный refactor — рецепт на месяц переноса с непредсказуемым результатом. Mitigation: **не делать** Eden migration в этом refactor'е. Оставить `apps/eden/` как в Variant 2.
:::

::: warning Risk 3 — Reversibility сильно падает
Variant 3 — это многосотенная diff с touched practically every Cargo.toml. `git revert` теоретически работает, но конфликты при rebase почти гарантированы. Mitigation: feature branch + один атомарный merge.
:::

::: warning Risk 4 — Build pipeline rewrite
`shell/package.json` scripts, electron-builder, possibly CI — всё переписывается. Time budget на verify ×3 vs Variant 2.
:::

### 5.7 Effort / Reversibility

- **Effort**: 4–7 рабочих дней. Если Eden Phase 6 включить — добавить ~2 недели.
- **Reversibility**: низкая. Кучи touched files, sharedtarget/. `git revert` рискован.

---

## 6. Раздел: Persistent extension user data (post-refactor design)

Это **post-refactor задача**, описание архитектуры на финальном layout'е (Variant 2 — `shell/` + `extensions/` на top-level).

### 6.1 Структура хранилища

```
%APPDATA%\Kosmos\
├── extensions/                         ◀ КОД extensions (replaceable)
│   └── <id>/
│       ├── manifest.json
│       ├── icon.png
│       ├── dist/
│       └── ...
├── extensions-data/                    ◀ USER DATA (persistent)
│   └── <id>/
│       ├── window-state.json           ◀ kepler shell пишет сам
│       ├── settings.json               ◀ extension пишет через userData API
│       ├── cache/                      ◀ extension пишет через userData API
│       └── ...
├── ark.db                              ◀ или spaces/<id>/ark.db
├── kepler.lock.json
├── selected-space.json
├── kepler-shell-settings.json          ◀ shell собственные settings (не extension data)
└── kepler-shell-window-state.json      ◀ shell launcher window state
```

### 6.2 Preload API (shell/electron/extension-preload.ts)

Расширить `window.kepler` namespace:

```ts
declare global {
  interface Window {
    kepler: {
      ark: { request; subscribe }; // существующее
      window: { close; minimize; maximize }; // существующее
      meta: { id }; // существующее
      userData: {
        // НОВОЕ
        readJson<T>(name: string): Promise<T | null>;
        writeJson<T>(name: string, value: T): Promise<void>;
        readFile(name: string): Promise<Buffer | null>;
        writeFile(name: string, data: Buffer | Uint8Array): Promise<void>;
        listFiles(): Promise<string[]>;
        deleteFile(name: string): Promise<void>;
      };
    };
  }
}
```

Семантика:

- `name` — это relative path внутри `%APPDATA%\Kosmos\extensions-data/<id>/`. Запрещены `..`, абсолютные пути, drive letter'ы (валидация в main).
- `id` резолвится из webContents → extension id (`webContentsToExtensionId` map, уже существует).
- Все ops async, идут через `ipcRenderer.invoke("kepler:user-data:*", { name, ... })`.
- Main implementation в `shell/electron/user-data.ts` (новый файл).

### 6.3 Window state — managed by shell

Extension **не управляет** своим window state. Shell сам:

1. При `openExtension(id)`:
   ```ts
   const state = await readJsonFromUserData(id, "window-state.json");
   const win = new BrowserWindow({
     width: state?.width ?? manifest.width,
     height: state?.height ?? manifest.height,
     x: state?.x, y: state?.y,
     ...
   });
   ```
2. При `win.on("close", ...)`:
   ```ts
   const bounds = win.getBounds();
   await writeJsonToUserData(id, "window-state.json", bounds);
   ```

### 6.4 Uninstall behavior

`shell/scripts/uninstall-extension.mjs`:

```
ext:uninstall <id>           # удаляет extensions/<id>/, СОХРАНЯЕТ extensions-data/<id>/
ext:uninstall <id> --purge   # удаляет ОБА
```

Аргументация: install/uninstall — для итерации над кодом. Случайная переустановка не должна обнулять settings/cache. Покинуть user data = ноль регрессий, place stuff обратно через install — всё на месте.

### 6.5 Install behavior

`shell/scripts/install-extension.mjs`:

- Копирует source → `%APPDATA%\Kosmos\extensions/<id>/` (atomic, как сейчас).
- **Не трогает** `%APPDATA%\Kosmos\extensions-data/<id>/`.
- При первой установке `extensions-data/<id>/` не создаётся явно — создаётся lazily при первом write через userData API.

### 6.6 Resolution chain (без изменений)

Существующий приоритет из `extension-host.ts`:

1. Dev: `extensions/<id>/` (repo) — если работает в dev режиме.
2. User: `%APPDATA%\Kosmos\extensions/<id>/`.
3. Bundled: `<resourcesPath>/extensions/<id>/`.

User data path (`extensions-data/<id>/`) — **независим от resolution chain**: всегда один и тот же, какая бы версия кода ни загрузилась.

### 6.7 Migration

На момент введения userData API ни один extension не пишет в свою папку (`spec.md` для installer-mvp подтверждает). Migration scenario:

- При первом старте Kepler с новой версией shell — ничего не делать. `extensions-data/` создаётся пустым.
- Никаких миграций существующих данных (их нет).

---

## 7. Раздел: Dashboard rework (post-refactor design)

### 7.1 Контекст

Текущий Dashboard (`extensions/dashboard/`) — read-only usage analytics: Overview + Sessions через `kepler.ark.request("get_usage_analytics")`. Пользователь хочет **новый Dashboard** — визуальный ARK browser.

### 7.2 Что показывает новый Dashboard

```
┌──────────────────────────────────────────────────────────────────────┐
│  Dashboard                                                            │
├──────────────┬───────────────────────────────────────────────────────┤
│ Sidebar      │ Main panel                                            │
│              │                                                       │
│ Object types │ ┌─────────────────────────────────────────────────┐  │
│  task_obj    │ │ Tab: Objects | Inspector | Activity              │  │
│  note_obj    │ ├─────────────────────────────────────────────────┤  │
│  game_obj    │ │                                                  │  │
│  time_entry  │ │  Objects-таблица (для выбранного типа)           │  │
│  tracked_app │ │  ──────────────────────────────────────────────  │  │
│  ...         │ │  id     | title              | created  | links │  │
│              │ │  abc... | "Buy groceries"    | 2 May    | 3     │  │
│ Activity ▼   │ │  def... | "Meeting notes"    | 1 May    | 0     │  │
│              │ │  ...                                             │  │
│              │ │                                                  │  │
│              │ │  Pagination / search by FTS5                     │  │
│              │ └─────────────────────────────────────────────────┘  │
└──────────────┴───────────────────────────────────────────────────────┘
```

### 7.3 Структура tabs

1. **Objects** (default tab):
   - Sidebar показывает `object_types` (читается через `kepler.ark.request("list_object_types")`).
   - При выборе типа — таблица объектов (`list_objects` с filter по `object_type`).
   - Columns динамические — берутся из `schemaJson` каждого type'а.
   - Row click → переключение на Inspector tab с selected object.

2. **Inspector** (на конкретном объекте):
   - JSON view properties.
   - Linked objects (`get_object_links`) — список с переходом по links.
   - Created/updated timestamps, HLC, sync metadata.
   - **Read-only**. Никаких edit/delete кнопок.

3. **Activity** (текущая dashboard функциональность):
   - Overview metrics (existing usage analytics).
   - Sessions list.
   - Это **раздел** Dashboard'а, не отдельная апка.

### 7.4 Что НЕ делает Dashboard

- ❌ ARK writes (ни create, ни update, ни delete). Только reads.
- ❌ Schema management (создание новых object_types). Это task для отдельной apk либо CLI.
- ❌ Cross-space view. Показывает только selected space.
- ❌ Bulk export / import. Это отдельная feature.

### 7.5 ARK API requirements (что должно быть в `@kosmos/ark`)

Существующее достаточно:

- `client.objects.list({ type, limit, offset, query? })`.
- `client.objects.get(id)`.
- `client.links.get(objectId)`.
- `client.objectTypes.list()`.

Если что-то отсутствует — это **отдельный proof loop** на расширение SDK / ark-core-rpc protocol. Не часть Dashboard rework'а.

### 7.6 Реализация

Создаётся новый extension `extensions/dashboard/` (либо переписывается existing extension). Стек: Vue 3 Vapor + `@kosmos/visuals` (как остальные extensions, без Tailwind). Pages:

```
extensions/dashboard/src/
├── App.vue                  layout: sidebar + main panel
├── main.ts
├── pages/
│   ├── ObjectsPage.vue      objects-таблица
│   ├── InspectorPage.vue    object detail
│   └── ActivityPage.vue     перенос текущего Overview/Sessions
├── composables/
│   ├── useObjectTypes.ts
│   ├── useObjectList.ts
│   └── useObjectInspector.ts
└── components/
    ├── ObjectTable.vue
    ├── ObjectRow.vue
    ├── LinkedObjects.vue
    └── ...
```

### 7.7 Связь с persistent user data

Dashboard может persist'ить:

- Selected object type (last opened).
- Column visibility / order.
- Table page size.

Через `window.kepler.userData.writeJson("ui-state.json", {...})`.

---

## 8. Раздел: usage-tracker → Kepler backend модуль

### 8.1 Цель

Сейчас:

```
[usage-tracker.exe]  ←─WS─→  [kepler-backend.exe]  ←─stdio─→  [ark-core-rpc.exe]
                        OR
[usage-tracker.exe]  ←─direct SQLite write─→  ark.db          (USAGE_TRACKER_USE_KEPLER=0)
```

После refactor'а:

```
[kepler-backend.exe]
  ├─ usage_tracker module (tokio::spawn)
  │   └─ windows_capture → events → ark_core::db::upsert_*
  └─ ark-core-rpc child  (для остальных операций)
```

Один процесс. Никакого WS-client'а. Direct calls в `ark_core::db` (in-process write — kepler-backend имеет `&'static ArkHost` или similar).

### 8.2 Модуль в backend'е

```
services/kepler-backend/src/
├── main.rs
├── lib.rs
├── ark_host.rs                       # supervisor + proxy ark-core-rpc
├── ws_server.rs
├── command_bus.rs
├── sync.rs
├── auth.rs
├── lock_file.rs
├── singleton.rs
├── protocol_version.rs
└── usage_tracker/                    ◀ НОВЫЙ МОДУЛЬ (mod usage_tracker)
    ├── mod.rs                        # public start()/stop() + конфиг
    ├── sampler.rs                    # портировано из services/usage-tracker/
    ├── windows_capture.rs            # портировано
    ├── model.rs                      # портировано
    ├── tracker.rs                    # портировано (core логика)
    └── config.rs                     # портировано
```

### 8.3 Lifecycle

В `kepler-backend/src/main.rs`:

```rust
async fn main() {
    // existing init: singleton, lock_file, ark_host, ws_server, sync...

    if config.usage_tracker_enabled {
        let ark_db_path = resolve_db_path();
        let cancel = CancellationToken::new();
        tokio::spawn(usage_tracker::run(ark_db_path, cancel.clone()));
        // cancel triggered on shutdown signal
    }

    // ... rest of main loop
}
```

### 8.4 Direct ARK writes

В сегодняшнем standalone usage-tracker есть два режима:

1. **WS клиент** (`USAGE_TRACKER_USE_KEPLER=1`) — `kepler_client.rs` → WS RPC к backend → backend пишет.
2. **Direct SQLite** (default до сих пор) — `db.rs` → прямой `rusqlite::Connection::open`.

После merge модуль использует **третий** вариант: in-process direct call в `ark_core::db::*`. Это безопасно потому что:

- Процесс один. Нет ARK lock contention с другим процессом.
- `ark_core::db::bump_sync_version_vector` вызывается естественно — write-boundary не нарушается.
- `kepler-backend.exe` уже владеет SQLite-соединением через `ark_host` (его proxy к ark-core-rpc). Modul может либо переиспользовать это соединение, либо открыть свою read-write connection в WAL mode.

::: warning Архитектурное решение по connection sharing
Текущий backend проксирует JSON через stdio к ark-core-rpc child. Это нужно потому что ark-core-rpc имеет много логики (sync, FTS, и т.п.). Usage_tracker модуль:

- **Опция A**: открывает свой `rusqlite::Connection` на ту же SQLite (WAL mode позволяет multiple writers через retry). Pros: simple. Cons: `bump_sync_version_vector` нужно вызвать вручную, надо обеспечить корректность HLC.
- **Опция B**: вызывает ark-core-rpc через тот же stdio channel что и WS клиенты. Не direct.
- **Опция C**: ark_core exposes "library" interface (`ark_core::db::*`) который usage_tracker импортирует, и kepler-backend ensures что только один writer активен.

**Рекомендуемая** — Опция C. `ark-core` crate уже path-depended из backend'а. Достаточно expose'нуть нужные helpers как public API и использовать их.
:::

### 8.5 Cargo.toml backend updates

```toml
# services/kepler-backend/Cargo.toml
[dependencies]
ark-core = { path = "../../packages/ark-core/rust" }   # уже есть
# usage-tracker deps добавляются:
sha2 = "0.10"
hostname = "0.4"
# windows-* фичи уже частично есть, добавить недостающие:
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_Security",
  "Win32_Security_Authorization",
  "Win32_System_Threading",
  "Win32_System_SystemInformation",   # ← добавить
  "Win32_UI_Input_KeyboardAndMouse",  # ← добавить
  "Win32_UI_WindowsAndMessaging",     # ← добавить
] }
```

### 8.6 Что происходит с `services/usage-tracker/`

После merge:

- **Variant 1/2**: переименовать в `services/usage-tracker-legacy/` или `legacy/usage-tracker/` (создав папку `legacy/`). НЕ удалять до Phase 8 verify.
- **Variant 3**: сразу в `legacy/usage-tracker/`.

Файлы которые НЕ переезжают в kepler-backend (потому что они для standalone use case):

- `main.rs` — entrypoint, не нужен.
- `singleton.rs` — kepler-backend имеет свой singleton.
- `kepler_client.rs` — WS клиент к backend'у, больше не нужен (мы внутри backend'а).
- `spool.rs` — offline queue для WS клиента. Не нужен (мы in-process).
- `db.rs` — direct SQLite open. Не нужен (используем ark_core::db).
- `installer/` — отдельный installer не нужен, поглощается shell installer'ом.

Файлы которые **переезжают**:

- `tracker.rs` — core логика.
- `sampler.rs` — system polling.
- `windows_capture.rs` — Win32 wrapper.
- `model.rs` — usage event types.
- `config.rs` — config struct.

### 8.7 Settings

В `kepler-shell-settings.json`:

```json
{
  "developerMode": false,
  "autoStart": true,
  "usageTracker": {
    "enabled": true,
    "sampleIntervalMs": 1000,
    "trackedApps": []
  }
}
```

Shell читает settings и при spawn backend'а передаёт env var типа `KOSMOS_USAGE_TRACKER=1`. Backend читает env, в `usage_tracker_enabled = env::var("KOSMOS_USAGE_TRACKER").is_ok()`.

### 8.8 Risk: regression

Нативная Win32 capture логика и lifecycle тонкие. Merge тратит риск регрессии. Mitigation:

- Keep `services/usage-tracker/` нетронутым в первой версии — merge делается параллельным branche'ом.
- Verify через A/B: standalone usage-tracker пишет в test DB, backend-merged version — в другой test DB, диф через sql query.
- Rollback path: `legacy/usage-tracker/` остаётся buildable как standalone exe до окончательного sign-off.

---

## 9. Что **не** делаем в этом refactor'е

Это эпично важно зафиксировать чтобы scope не плыл:

1. ❌ **Renaming `@kosmos/ark` → что-то ещё.** Только что swap'нут brand, второй swap = двойной риск.
2. ❌ **Renaming `kepler-shell` → `kepler-host` / `kepler-launcher`.** Имя зафрахтовано в docs / installer / NSIS.
3. ❌ **Phase 6 Eden миграция в extension.** Eden остаётся `apps/eden/` (Variant 1/2) до отдельного proof loop'а. В Variant 3 — упоминается, но рекомендуется отделить.
4. ❌ **Удаление `services/kepler-watcher/`.** Малый watchdog (3 deps lines), оставляется на случай если понадобится (особенно при relaunch backend'а после крэша).
5. ❌ **Renaming git remote / monorepo dir.** Пользователь не просил.
6. ❌ **Cargo workspace setup** в Variant 1/2. Только Variant 3 (опционально). Cargo workspace — отдельная win, но добавляет surface для bugs.
7. ❌ **Auto-update mechanism.** Phase 8 Auto-update — отдельный проект.
8. ❌ **Dashboard implementation.** Этот документ описывает **target design**. Реализация — отдельный proof loop после refactor'а.
9. ❌ **usage-tracker merge implementation.** Этот документ описывает **target design**. Реализация — отдельный proof loop после refactor'а.

---

## 10. Summary comparison

| Параметр                                  | Variant 1 (минимальный)                           | Variant 2 (средний)                                              | Variant 3 (радикальный)                                                  |
| ----------------------------------------- | ------------------------------------------------- | ---------------------------------------------------------------- | ------------------------------------------------------------------------ |
| **Effort**                                | 2–4 часа                                          | 1–2 рабочих дня                                                  | 4–7 рабочих дней (+ 2 недели если Eden)                                  |
| **Risk (junction discharge)**             | Низкий                                            | Средний (больше mv операций)                                     | Высокий (Cargo workspace + Eden)                                         |
| **Risk (build pipeline)**                 | Минимальный                                       | Средний (vite/electron-builder/scripts)                          | Высокий (Cargo workspace, electron-builder, scripts, multiple pipelines) |
| **Reversibility**                         | Высокая (только `git rm`)                         | Средняя (много `git mv` + bun.lock)                              | Низкая (Cargo workspace + multi-Crate edits)                             |
| **Aligns with persistent extension data** | Да (нейтрально — не блокирует)                    | Да (extensions/ top-level упрощает mental model)                 | Да                                                                       |
| **Aligns with new Dashboard**             | Да (нейтрально)                                   | Да (extensions/dashboard/ — естественное место)                  | Да                                                                       |
| **Aligns with usage-tracker merge**       | Да (нейтрально)                                   | Да                                                               | Да (делается в этом же PR)                                               |
| **Reduces apps/ confusion**               | Частично (убирает дубликаты)                      | Полностью (top-level разделение по ролям)                        | Полностью (+ Rust vs TS разделение)                                      |
| **Eden migration coupling**               | Не связано                                        | Не связано (eden остаётся в apps/)                               | Связано (Eden принудительно мигрирует)                                   |
| **Cognitive simplicity for newcomer**     | Средняя (всё ещё `apps/kepler-shell/extensions/`) | Высокая (`shell/`, `extensions/`, `services/` — самообъясняющие) | Высокая, но Cargo workspace добавляет ментальный налог                   |
| **Bun.lock churn**                        | Минимальный                                       | Большой                                                          | Очень большой                                                            |
| **Cargo.lock churn**                      | Нулевой                                           | Нулевой                                                          | Большой                                                                  |

---

## 11. Рекомендация

**Variant 2.** Он закрывает все жалобы пользователя на путаницу (один уровень — одна роль; нет дубликатов; legacy явно удалён), при этом не втягивает risky scope (Eden Phase 6, Cargo workspace). Effort умеренный — 1-2 дня с полноценным verify. Reversibility — приемлемая.

Variant 1 — fallback если на refactor сейчас нет окна (можно сделать за день, частично решает проблему).

Variant 3 — отложить до момента когда продукт станет более стабильным; нет смысла платить за shared Cargo workspace и Eden migration в один присест когда они не блокируют друг друга. Cargo workspace сделается отдельно когда деплои стабилизируются и cold build time станет заметной болью.

После Variant 2 рекомендуется в следующих proof loop'ах в указанном порядке:

1. **`extensions-data/`** + `userData` preload API (см. секция 6).
2. **usage-tracker merge** в kepler-backend (см. секция 8).
3. **Dashboard rework** (см. секция 7) — последним, потому что выигрывает от `userData` API и от merged usage-tracker (Activity tab читает usage напрямую от того же ARK).

---

## Appendix A. Полный pre-flight checklist для миграции (Variant 2)

```powershell
# 1. Sanity
git status --short                           # пусто
git status --short packages/                 # ОБЯЗАТЕЛЬНО пусто
bun run ark:smoke                            # PASS
bun run --cwd apps/kepler-shell typecheck    # PASS
bun run --cwd apps/kepler-shell build:extensions  # PASS

# 2. Stash anything untracked
git stash --include-untracked

# 3. Снести node_modules
$dirs = @(
  'apps\kepler-shell\node_modules',
  'apps\kepler-shell\extensions\dashboard\node_modules',
  'apps\kepler-shell\extensions\horologion\node_modules',
  'apps\kepler-shell\extensions\delphi\node_modules',
  'apps\kepler-shell\extensions\arrancador\node_modules',
  'apps\dashboard\node_modules',
  'apps\horologion\node_modules',
  'apps\arrancador\node_modules',
  'apps\delphi\ts\node_modules',
  'apps\eden\ts\node_modules',
  'apps\ark-service\node_modules',
  'apps\kepler\target',
  'node_modules'
)
foreach ($d in $dirs) { if (Test-Path $d) { Remove-Item -Recurse -Force $d } }

# 4. Confirm packages/ junction targets safe
Get-ChildItem packages\kosmos-ark -Recurse | Where-Object { $_.LinkType } | ForEach-Object { Write-Host "Junction: $_" }
# Should print nothing — junction'ы должны быть только В apps/*/node_modules, не НАОБОРОТ.

# 5. Proceed with migration steps from Variant 2
```

## Appendix B. Post-migration smoke matrix

```powershell
bun install                                  # пересоздать junction'ы
bun run docs:sync                            # обновить AGENTS.md / CLAUDE.md
bun run ark:guard:writes
bun run ark:smoke

# Rust
cargo build --manifest-path services/kepler-backend/Cargo.toml --lib
cargo test --manifest-path services/kepler-backend/Cargo.toml --lib
cargo build --manifest-path services/kepler-backend/Cargo.toml --bin kepler-backend
cargo build --manifest-path packages/ark-core/rust/Cargo.toml --bin ark-core-rpc

# TS
bun run --cwd packages/kosmos-ark typecheck
bun run --cwd shell typecheck
bun run --cwd shell build:extensions

# Manual smoke (без e2e скриптов)
bun run --cwd shell dev:kepler
#   - launcher открывается на Ctrl+Shift+K
#   - все 4 extensions открываются из launcher'а
#   - tray menu работает
#   - settings window открывается
#   - закрытие extension'а не падает

# Package
bun run --cwd shell build                    # release/Kepler Setup X.Y.Z.exe
# Install + run installed binary, verify same behaviour.
```

---

**Конец документа.**

### Critical Files for Implementation

Файлы наиболее критичные для последующей реализации refactor'а (по любому из вариантов):

- `D:/Personal/Hobby/Coding/kepler/apps/kepler-shell/package.json` — electron-builder extraResources и build/dev scripts с manifest-path; ломается при любом перемещении shell или extensions.
- `D:/Personal/Hobby/Coding/kepler/apps/kepler-shell/vite.extensions.config.mjs` — discoverVueExtensions + aliasи `@kosmos/ark`/`@kosmos/visuals` со скейпом `../../packages/`; пересчитывается при перемещении extensions/.
- `D:/Personal/Hobby/Coding/kepler/apps/kepler-shell/vite.config.mjs` — те же aliasы для главного renderer/electron build'а.
- `D:/Personal/Hobby/Coding/kepler/scripts/sync-agents-docs.mjs` — single source of truth для AGENTS.md mappings; обновляется на каждое перемещение app/service/package.
- `D:/Personal/Hobby/Coding/kepler/package.json` — root bun workspaces glob; добавляются/убираются members при изменении layout'а.
