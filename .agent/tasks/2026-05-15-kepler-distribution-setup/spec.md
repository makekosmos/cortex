# Kepler distribution setup — auto-update через GitHub Releases

## Цель

Production Kepler приложение должно обновляться **без переустановки**:
1. Юзер скачивает `Kepler-Setup-X.Y.Z.exe` с GitHub Release.
2. Запускает, использует.
3. Через N часов / при следующем старте — autoUpdater проверяет новый
   release, скачивает diff, при следующем рестарте применяет.
4. Никакого ручного re-install для minor / patch updates.

## Архитектура

```
┌──────────────────────────────────────────────────────────┐
│ Dev machine (Kazui)                                       │
│  bun run --cwd shell build                                │
│   ↓                                                       │
│  electron-builder packs Kepler-Setup-X.Y.Z.exe + latest.yml│
│   ↓ publish: github                                       │
│  Pushes assets в GH release tag v{X.Y.Z}                  │
└──────────────────────────────────────────────────────────┘
                       │
                       ↓ (GitHub-hosted)
┌──────────────────────────────────────────────────────────┐
│ github.com/<OWNER>/kepler-releases (PUBLIC repo)          │
│  Releases:                                                │
│   v0.1.0/                                                 │
│     Kepler-Setup-0.1.0.exe                                │
│     latest.yml ← metadata file electron-updater reads     │
│   v0.1.1/ ...                                             │
└──────────────────────────────────────────────────────────┘
                       │
                       ↓ HTTPS pull
┌──────────────────────────────────────────────────────────┐
│ End-user machine                                          │
│  Kepler.exe v0.1.0 (installed)                            │
│   on app.whenReady():                                     │
│     autoUpdater.checkForUpdatesAndNotify()                │
│     setInterval(check, N hours)                           │
│   ↓ if new release detected                               │
│   downloads installer в %TEMP%\kepler-updater\            │
│   ↓ at next quit (или prompt)                             │
│   applies update, restarts с новой версией                │
└──────────────────────────────────────────────────────────┘
```

## Открытые вопросы (decide перед execution)

1. **GitHub repo name + owner.** Default: `github.com/ksanrse/kepler-releases`
   (matches существующий `kepler` repo owner). Confirmed?
2. **Public или Private?** Public = simpler — electron-updater тянет без
   auth token на client side. Private = надо передавать GH_TOKEN на каждой
   installed machine — нерабочий вариант для open distribution.
   **Recommendation: Public.**
3. **Update interval.** VS Code = 1 hour, Slack = 4 hours. Recommend:
   - Start-up check (immediate)
   - Periodic check каждые 6 hours
   - Configurable через Settings (Phase 2)
4. **Code signing.** Без подписи Windows SmartScreen покажет «Unknown
   Publisher» при первом запуске. Юзер кликает «Run anyway». Это OK для
   solo-dev стадии. Signing certificate стоит ~$300/year (DigiCert) или
   бесплатный self-signed (но всё равно warning). **Recommendation:
   skip signing для v0.x, add when stable + paying users.**
5. **Auto-download vs prompt-first.** Default electron-updater behaviour:
   download silently (background) → notification «update готов, рестарт?»
   На restart applies. **Recommendation: дефолт.**
6. **Channels (stable / beta / dev).** Пока один channel. Если позже —
   `channel: "beta"` в `latest.yml` для opt-in beta.

## Acceptance criteria

### Phase A — Release repo + publish config

- **AC1**: PUBLIC repo `github.com/<OWNER>/kepler-releases` создан.
  Initial commit с README объясняющим что это binary-only distribution
  channel.
- **AC2**: `shell/package.json` build config расширен:
  ```json
  "build": {
    "publish": [{
      "provider": "github",
      "owner": "<OWNER>",
      "repo": "kepler-releases",
      "releaseType": "release"
    }],
    "win": { ... },
    "nsis": { ... }
  }
  ```
- **AC3**: GH_TOKEN env var на dev machine (`personal access token` с
  scope `repo` для приватного, `public_repo` для публичного). НЕ
  commit'ить в repo, добавить в `.gitignore` если случайно.
- **AC4**: `bun run --cwd shell build` собирает installer **и пушит**
  его в release. Если GH_TOKEN отсутствует — fail с понятным error.

### Phase B — electron-updater integration

- **AC5**: `electron-updater` dep добавлен в `shell/package.json`
  dependencies. `bun install` clean.
- **AC6**: `shell/electron/main.ts` импортирует `{ autoUpdater } from
  "electron-updater"`. В `whenReady`:
  ```ts
  // Skip в dev / test mode (KOSMOS_TEST_MODE, VITE_DEV_SERVER_URL).
  if (!isDev && !isTest) {
    autoUpdater.checkForUpdatesAndNotify();
    setInterval(() => autoUpdater.checkForUpdatesAndNotify(), 6 * 60 * 60 * 1000);
  }
  ```
- **AC7**: autoUpdater events logged для debug (update-available,
  download-progress, update-downloaded, error). Прокинуть `error` в
  Settings → System log если есть, или просто console.error пока.
- **AC8**: При получении `update-downloaded` event — показать notification
  «Kepler X.Y.Z скачан, перезапустить?» с двумя кнопками. Click yes →
  `autoUpdater.quitAndInstall()`.

### Phase C — End-to-end verification

- **AC9**: На SECOND machine (или VM, или fresh user profile) установить
  скачанный `Kepler-Setup-0.1.0.exe`. Запустить — приложение работает.
- **AC10**: На dev machine bump `shell/package.json` version → 0.1.1.
  `bun run --cwd shell build` пушит v0.1.1 в release repo.
- **AC11**: На second machine рестартанутый Kepler в течение ~1 минуты
  detect'ит update, downloads, prompts. Restart → версия v0.1.1.

### Phase D — Docs

- **AC12**: `docs-site/concepts/distribution.md` — описание flow:
  - Что Kepler distributed через GitHub Releases.
  - Как сделать release (`bun run --cwd shell build`).
  - Как добавить changelog к release.
  - Что юзеры видят (silent download + restart prompt).
  - Settings → «Проверить обновления вручную» (Phase 2 — пока через CLI).
- **AC13**: `docs-site/agents/forbidden.md` дополнен:
  - ❌ Коммитить GH_TOKEN в repo / settings.
  - ❌ Менять provider с `github` на что-то иное без обсуждения.
  - ❌ Пушить release tag manually без `electron-builder publish` —
    он генерирует `latest.yml` с SHA512 hash, без него
    electron-updater не сможет validate.

### Phase E — CI / tooling (опционально)

- **AC14 (optional)**: GitHub Actions workflow на push в `kepler` main →
  trigger `bun run build`, push release. Solo dev пока всё локально —
  defer to когда нужно.
- **AC15 (optional)**: Settings → «Проверить обновления» button —
  manual trigger `autoUpdater.checkForUpdates()`. Phase 2.

## Workflow (план коммитов)

```
1. chore(shell): electron-updater dep + build.publish github config
2. feat(kepler-shell): autoUpdater integration в main.ts
3. test(e2e): autoUpdater skipped в test mode (regression check)
4. docs: distribution.md + forbidden.md updates
5. (manual) первый release v0.1.0 → kepler-releases
6. (manual) verify on second machine
7. (manual) bump → 0.1.1 → verify auto-update flow
```

## Ограничения

- **НЕ** включать autoUpdater в dev (VITE_DEV_SERVER_URL) или test
  (KOSMOS_TEST_MODE) — расход GitHub API + удивление при ручной
  разработке.
- **НЕ** force auto-restart без user consent.
- **НЕ** загружать update если на metered connection (Windows API
  доступна, но Phase 2 — пока default electron-updater behaviour).
- Если update download fails (network, GitHub down) — silent, retry next
  interval. Не показывать error баннер если update не critical.

## Pre-execution checklist (для user'а)

Прежде чем агент начнёт implement:

1. [ ] Создать публичный repo `github.com/<OWNER>/kepler-releases` (пустой
       OK, agent внесёт README).
2. [ ] Создать GH Personal Access Token с `public_repo` scope.
3. [ ] Set env `GH_TOKEN=<token>` или `GITHUB_TOKEN=<token>` (electron-builder
       читает оба).
4. [ ] Подтвердить **OWNER** name (`ksanrse` или другой?).
5. [ ] Подтвердить repo name `kepler-releases` (или другой?).
6. [ ] Decision: code signing skip (рекомендую да на старте).

После этого agent execute'ит phases A-D atomic commits, плюс manually
тестируем D.

## Risks

| Risk | Mitigation |
|---|---|
| GitHub rate limit (60 req/hour без auth) | autoUpdater использует releases API, ~1 req/check × 4 check/day = 4 req. Below limit. Auth-via-token enable'ит 5000/hour если надо. |
| Windows SmartScreen warning без signed installer | User кликает «Run anyway». Documented в README of kepler-releases. Add signed cert later. |
| Update applies во время unsaved work | electron-updater по дефолту download silently, prompt at user-convenient time. quitAndInstall ждёт user click. Safe. |
| Bricked update (новая версия крешится на старте) | Юзер может re-install старую `Kepler-Setup-0.1.0.exe` ручным download. Phase 2 — auto-rollback. |
| Release repo accidentally публичный с приватным token | GH PAT scope minimal (только `public_repo`), не хранится в repo. Rotate token if leaked. |
