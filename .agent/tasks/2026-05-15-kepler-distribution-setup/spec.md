# Kosmos distribution — Raycast-style 2-repo model

## Цель

1. **Kepler launcher** — distributed как Windows installer через GitHub
   Releases. autoUpdater на старте + каждые 6 часов. Юзер не переинсталлит
   вручную.
2. **Kosmos extensions** — Raycast-style marketplace. Отдельный публичный
   repo с `.kext` artifacts + `catalog.json`. Лаунчер тянет catalog,
   показывает доступные / новые extensions, устанавливает через
   существующий Wave 1 `.kext` installer flow.
3. Source code остаётся в **`ksanrse/kepler`** monorepo. Distribution
   repos — **binary only**.

## Brand convention (per CLAUDE.md)

| Уровень            | Имя                                                             |
| ------------------ | --------------------------------------------------------------- |
| Ecosystem          | **Kosmos**                                                      |
| Launcher app       | **Kepler**                                                      |
| Distribution repos | `kepler-releases` (launcher), `kosmos-extensions` (marketplace) |

## Архитектура

```
┌──────────────────────────────────────────────────────────────────┐
│ Dev machine (ksanrse)                                             │
│  bun run --cwd shell build              → kepler-releases v0.1.0  │
│  bun run ext:publish horologion         → kosmos-extensions/      │
│                                            horologion-v0.3.0      │
│                                          + auto-updated catalog.json│
└──────────────────────────────────────────────────────────────────┘
                       │
                       ↓ GitHub-hosted, public
┌──────────────────────────────────────────────────────────────────┐
│ github.com/yoso-industries/kepler-releases (PUBLIC)                       │
│  Releases:                                                        │
│   v0.1.0/                                                         │
│     Kepler-Setup-0.1.0.exe   ← installer (lean: shell + ark only) │
│     latest.yml               ← electron-updater метадата          │
└──────────────────────────────────────────────────────────────────┘
                       │
                       ↓ HTTPS pull (autoUpdater)
┌──────────────────────────────────────────────────────────────────┐
│ End-user machine                                                  │
│  Kepler.exe (launcher + ARK backend, БЕЗ bundled extensions)      │
│   on whenReady():                                                 │
│     autoUpdater.checkForUpdatesAndNotify()  ← kepler-releases     │
│     setInterval(check, 6h)                                        │
│   Settings → Extensions → Marketplace:                            │
│     fetch https://raw.githubusercontent.com/yoso-industries/              │
│       kosmos-extensions/main/catalog.json                         │
│     show grid: Horologion, Делphi, Arrancador, ...                │
│     [Install] → download .kext from kosmos-extensions release     │
│     [Update] → бadge if installed.version < catalog.version       │
└──────────────────────────────────────────────────────────────────┘
                       ↑
                       │ HTTPS pull
┌──────────────────────────────────────────────────────────────────┐
│ github.com/yoso-industries/kosmos-extensions (PUBLIC, RAYCAST-STYLE)      │
│  catalog.json              ← auto-generated на каждом publish     │
│  extensions/               ← metadata + README + icon per ext      │
│    horologion/{manifest.json, icon.png, README.md}                │
│    delphi/...                                                     │
│    arrancador/...                                                 │
│  README.md                  ← submission guide для future contrib  │
│  Releases (per-extension tagged):                                 │
│    horologion-v0.3.0/                                             │
│      horologion-0.3.0.kext                                        │
│    delphi-v0.2.1/                                                 │
│      delphi-0.2.1.kext                                            │
└──────────────────────────────────────────────────────────────────┘
```

## Decisions (confirmed user)

| #   | Решение                        | Value                                                                                                                                                |
| --- | ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Owner                          | `yoso-industries` (GitHub organization; commits всё ещё под `ksanrse` user)                                                                          |
| 2   | Repo names                     | `kepler-releases` + `kosmos-extensions`                                                                                                              |
| 3   | Visibility                     | оба public                                                                                                                                           |
| 4   | autoUpdater interval           | start-up + 6h                                                                                                                                        |
| 5   | Marketplace check interval     | start-up + 24h (catalog reasonably static)                                                                                                           |
| 6   | Code signing                   | skip пока v0.x (SmartScreen warning OK)                                                                                                              |
| 7   | Channels                       | один stable, beta/dev — defer                                                                                                                        |
| 8   | Mainставлер bundled extensions | **No** — lean Kepler installer без extensions. Extensions устанавливаются marketplace-driven. (Если переключаемся на bundled — это решение Phase 2.) |

## Acceptance criteria

### Phase A — Kepler launcher distribution

- **AC1**: `github.com/yoso-industries/kepler-releases` создан, public, пустой.
- **AC2**: `shell/package.json` build config:
  ```json
  "build": {
    "publish": [{
      "provider": "github",
      "owner": "yoso-industries",
      "repo": "kepler-releases",
      "releaseType": "release"
    }],
    "extraResources": [
      "../target/release/kepler-backend.exe",
      "../target/release/ark-core-rpc.exe"
      // ← extensions исключены, marketplace flow обеспечит установку
    ],
    ...
  }
  ```
- **AC3**: `GH_TOKEN` env установлен на dev machine (PAT `public_repo` scope).
- **AC4**: `bun run --cwd shell build` собирает + пушит installer в release.
  Если GH_TOKEN missing — fail с понятным error.

### Phase B — electron-updater интеграция

- **AC5**: `electron-updater` dep в `shell/package.json`. `bun install` clean.
- **AC6**: `shell/electron/main.ts` импорт + integration:

  ```ts
  import { autoUpdater } from "electron-updater";

  // Skip в dev / test mode.
  if (!isDev && process.env.KOSMOS_TEST_MODE !== "1") {
    autoUpdater.checkForUpdatesAndNotify();
    setInterval(() => autoUpdater.checkForUpdatesAndNotify(), 6 * 60 * 60 * 1000);
  }
  ```

- **AC7**: События logged (update-available, download-progress, downloaded, error).
- **AC8**: При `update-downloaded` — native notification «Kepler X.Y.Z готов,
  перезапустить?» → `autoUpdater.quitAndInstall()` если yes.

### Phase C — Kosmos extensions marketplace repo

- **AC9**: `github.com/yoso-industries/kosmos-extensions` создан, public, пустой.
- **AC10**: Initial commit в `kosmos-extensions` через `ext:publish-all`:
  - `README.md` с submission guide.
  - `extensions/<id>/` папки с manifest.json + README.md + icon.png — копии из
    `extensions/<id>/` source repo (только metadata, не код).
  - `catalog.json` с пустым `extensions: []`.

### Phase D — `ext:publish` workflow scripts

- **AC11**: `shell/scripts/publish-extension.mjs`:
  ```
  Args: <extension-id>
  Steps:
  1. Read extensions/<id>/manifest.json → version
  2. vite build extension → dist/
  3. zip into <id>-<version>.kext (используя shell/scripts/zip-utils.mjs from .kext infra)
  4. Compute SHA-256 hash.
  5. gh release create <id>-v<version> -R yoso-industries/kosmos-extensions <id>-<version>.kext
  6. Re-generate catalog.json (см. AC12) → commit + push в kosmos-extensions.
  ```
- **AC12**: `shell/scripts/generate-catalog.mjs`:
  - Fetch GitHub releases от `kosmos-extensions` через `gh api`.
  - Group by extension-id (prefix), pick highest semver per group.
  - Read manifest.json + icon.png paths from `extensions/<id>/` в kosmos-extensions repo.
  - Output `catalog.json`:
    ```json
    {
      "schemaVersion": 1,
      "updatedAt": "ISO timestamp",
      "extensions": [
        {
          "id": "horologion",
          "name": "Horologion",
          "description": "...",
          "author": "yoso-industries",
          "version": "0.3.0",
          "keplerApiVersion": "^1.0.0",
          "iconUrl": "https://raw.githubusercontent.com/yoso-industries/kosmos-extensions/main/extensions/horologion/icon.png",
          "downloadUrl": "https://github.com/yoso-industries/kosmos-extensions/releases/download/horologion-v0.3.0/horologion-0.3.0.kext",
          "sha256": "abc123..."
        }
      ]
    }
    ```
- **AC13**: `bun run ext:publish horologion` end-to-end создаёт release +
  обновляет catalog.json.
- **AC14**: `bun run ext:publish-all` итерирует всё extensions в monorepo.

### Phase E — Launcher Marketplace UI

- **AC15**: `shell/src/views/SettingsView.vue` → Extensions tab расширен:
  - Subtab «Установленные» (existing): user-installed list + Revert.
  - Subtab **«Маркетплейс»** (NEW): fetch catalog, render grid из extension cards
    (icon, name, version, description, [Install]/[Update]/[Installed] state).
- **AC16**: IPC `kepler:extension:catalog:fetch()` в main — реальный HTTPS call к
  `https://raw.githubusercontent.com/yoso-industries/kosmos-extensions/main/catalog.json`,
  cache 1 hour в memory. Force-refresh button — re-fetch.
- **AC17**: `kepler:extension:install:fromUrl(url)` — download .kext from URL в
  tmp, validate sha256 против catalog'ового, потом передать в existing
  `installFromPath` (Wave 1 infrastructure).
- **AC18**: Periodic catalog check каждые 24h — compare installed versions vs
  catalog.json latest. Если новее → badge в Settings → Extensions «N updates
  available».

### Phase F — End-to-end verification

- **AC19**: На SECOND machine (или fresh user profile):
  - Install `Kepler-Setup-0.1.0.exe` from kepler-releases
  - Открыть Settings → Extensions → Marketplace → install Horologion
  - Horologion открывается, работает (Pomodoro session backend пишет в DB)
- **AC20**: На dev machine bump Horologion version в `extensions/horologion/manifest.json` →
  `bun run ext:publish horologion` → новый release + updated catalog.
- **AC21**: На second machine рестартанутый Kepler в течение 24h (или manual
  refresh) detect'ит update Horologion → install → Horologion новая версия.
- **AC22**: Bump Kepler version в `shell/package.json` → `bun run --cwd shell
build` пушит installer → second machine получает autoUpdater notification.

### Phase G — Docs + governance

- **AC23**: `docs-site/concepts/distribution.md` — full flow описание:
  - 2-repo model
  - autoUpdater для launcher
  - Marketplace flow для extensions
  - Submission guide (для future community contributors)
- **AC24**: `docs-site/agents/forbidden.md` дополнен:
  - ❌ Коммитить GH_TOKEN.
  - ❌ Bundle extensions в Kepler installer (нарушает marketplace pattern).
  - ❌ Pushing release tag manually без `electron-builder publish` /
    `ext:publish` — генерируют SHA256 hash и `latest.yml`.
  - ❌ Менять wire format `catalog.json` без bump `schemaVersion` —
    installed Kepler'ы продолжат читать старый format.
- **AC25**: `kosmos-extensions/README.md` — submission guide:
  - Fork main `kepler` repo, write extension в `extensions/<id>/`
  - PR с manifest + source. Maintainer (`ksanrse`) reviews + merges
  - На merge → `ksanrse` запускает `bun run ext:publish <id>` →
    marketplace получает new release

## Workflow (план коммитов)

```
A1. chore(shell): electron-updater dep + build.publish github config
A2. feat(kepler-shell): autoUpdater integration в main.ts (dev/test skip)
A3. test(e2e): regression — autoUpdater skipped в test mode
A4. (manual) GH_TOKEN env + первый build → publishes v0.1.0 в kepler-releases

B1. feat(shell/scripts): publish-extension.mjs + generate-catalog.mjs
B2. feat(shell): IPC kepler:extension:catalog:fetch + installFromUrl
B3. feat(kepler-shell): Settings → Extensions → Marketplace subtab UI
B4. test(e2e): marketplace flow — catalog fetch + install + updateflagged

C1. (manual) initial kosmos-extensions seed: README + extensions metadata
C2. (manual) ext:publish-all → первая партия releases
C3. (manual) catalog.json первый commit в main

D1. docs: concepts/distribution.md + submission guide + forbidden updates

E. (manual verification) on second machine — install Kepler → marketplace
   → install ext → bump → auto-update
```

## Ограничения

- НЕ bundle'ить extensions в Kepler installer — нарушает marketplace flow.
- НЕ skip'ать `electron-builder publish` (он генерирует sha256 + latest.yml).
- НЕ commit'ить GH_TOKEN в repo. Если случайно — rotate token immediately.
- НЕ менять wire format `catalog.json` без `schemaVersion` bump.
- НЕ автоматически Install extension без consent (banner / list но не
  silent download for unfamiliar code).

## Pre-execution checklist (user side)

1. [ ] Создать публичный `github.com/yoso-industries/kepler-releases` (пустой).
2. [ ] Создать публичный `github.com/yoso-industries/kosmos-extensions` (пустой).
3. [ ] GH PAT с `public_repo` scope (один token на оба repo).
4. [ ] `$env:GH_TOKEN = "ghp_..."` (или `setx GH_TOKEN ...` permanent).
5. [ ] `gh` CLI установлен и `gh auth login` сделан (используется в
       publish-extension.mjs для release create). Если нет — agent
       fallback'нёт на `curl + GitHub REST API`.
6. [ ] Сказать «go» — agent execute'ит phases A1-D1 atomic commits.
       Phase A4 / C1-C3 / E — manual или semi-automated, в зависимости от
       gh CLI availability.

## Risks

| Risk                                | Mitigation                                                                                                                       |
| ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| GH rate limit                       | autoUpdater check ~4/day. Catalog check ~1/day. User load: 5 req/day. Unauth limit 60/h → OK. С PAT 5000/h.                      |
| SmartScreen warning                 | Documented в README. Кликнуть «Run anyway». Cert ~$300/y когда стабильно.                                                        |
| Catalog drift                       | `schemaVersion` field + Kepler tolerant к unknown fields. Breaking change → bump version + maintain backward read для 1 release. |
| Brick после update                  | Manual download предыдущей `Kepler-Setup-X.Y.Z.exe` from kepler-releases. Phase 2: auto-rollback.                                |
| Token leak                          | Scope minimal (только public_repo). Не хранить в repo. Rotate если exposed.                                                      |
| Extension malware (когда community) | Phase 3 — code review process. Сейчас все extensions свои → trust ok.                                                            |

## Out of scope (Phase 2+)

- Community PR-based submission flow (когда появится первый внешний
  contributor)
- Beta channel
- Auto-rollback после bad release
- macOS / Linux installers
- Mobile companion sync через marketplace
- Code signing certificate
- GitHub Actions для auto-build на push (solo dev — всё локально пока)
- Star / install counts / rating (никакого backend кроме GitHub raw)
