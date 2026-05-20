# Instance slots — изоляция prod / dev / multi-dev

::: tip TL;DR
Каждой запущенной копии Kepler присваивается **slot** (`prod`, `dev`, `dev-<x>`, `test-<x>`). На его основе детерминированно derive'ятся **все** dimensions, которые могут конфликтовать между параллельными инстансами: Electron userData, ARK data dir, productName, hotkey, autoupdater, HKCU autorun. Результат — installed prod Kepler можно использовать как личный софт **одновременно** с одним или несколькими активными dev-сессиями.
:::

## Зачем

Пользователь параллельно разрабатывает Kepler и **сам им пользуется** (личные заметки, задачи, pomodoro). До этой системы:

- `app.requestSingleInstanceLock()` был общим для prod и dev → второй инстанс получал focus-redirect и сразу exit.
- Electron `userData` определялся через `productName` → prod и dev писали в один `%APPDATA%\Kepler\` (settings, autoupdater state, window cache).
- `globalShortcut(Ctrl+Shift+K)` пытались взять оба инстанса.
- HKCU autorun из dev добавлял мусор в реестр (path указывал на `electron.exe` из node_modules).

Дополнительно — мульти-агентная разработка: разные агенты в разных git worktree должны параллельно гонять Kepler с разными ветками, без перекрёстных эффектов.

## Слоты (соглашение)

| slot | trigger | Electron userData | ARK dataDir | productName | hotkey | autoupdater | autorun |
|---|---|---|---|---|---|---|---|
| `prod` (default) | installed `Kepler.exe` | `%APPDATA%\Kepler\` | `%APPDATA%\Kosmos\` | Kepler | `Alt+Space` | on | разрешён |
| `dev` | `VITE_DEV_SERVER_URL` set, нет `KEPLER_INSTANCE` | `%APPDATA%\Kepler-dev\` | `%APPDATA%\Kosmos-dev\` | Kepler [dev] | `` Alt+` `` | off | запрещён |
| `dev-<x>` | `KEPLER_INSTANCE=dev-<x>` (per-worktree `.env.local`) | `%APPDATA%\Kepler-dev-<x>\` | `%APPDATA%\Kosmos-dev-<x>\` | Kepler [dev-<x>] | disabled | off | запрещён |
| `test-<x>` | `KOSMOS_DATA_DIR` set (Playwright helper) | `<KOSMOS_DATA_DIR>/userdata/` | `KOSMOS_DATA_DIR` (absolute) | Kepler [test] | disabled | off | запрещён |

`<x>` — `[a-z0-9][a-z0-9-]*`. Соглашение по именам: `dev-a`, `dev-b`, `dev-eden`, `dev-issue-42`.

### Resolution priority

`shell/electron/instance.ts::resolveInstance()`:

1. `KEPLER_INSTANCE` env (explicit override). Валидируется regex.
2. Иначе если `KOSMOS_DATA_DIR` set → `test-<basename(KOSMOS_DATA_DIR)>`.
3. Иначе если `VITE_DEV_SERVER_URL` set → `dev`.
4. Иначе → `prod`.

## Где это применяется в коде

Single source of truth: **`shell/electron/instance.ts`**.

`shell/electron/main.ts` в самом верху (до `requestSingleInstanceLock`):

```ts
import { resolveInstance, applyInstanceToApp } from "./instance";
const KEPLER_INSTANCE = resolveInstance();
applyInstanceToApp(KEPLER_INSTANCE);  // app.setName + app.setPath('userData') + setAppUserModelId
```

Дальше:
- `keplerDataDir()` в `data-dir.ts` делегирует в `resolveInstance().dataDir`.
- `crashReporter.start({ productName: KEPLER_INSTANCE.productName, ... })`.
- Backend spawn env: `KOSMOS_DATA_DIR=<dataDir>`, `KEPLER_INSTANCE=<slot>`.
- Tray tooltip: `KEPLER_INSTANCE.productName`.
- `setupAutoUpdater({ isDev: !KEPLER_INSTANCE.autoupdaterEnabled })`.
- `startPeriodicCatalogCheck()` — skip если `!KEPLER_INSTANCE.periodicMarketplaceCheckEnabled`.
- `globalShortcut.register(...)` — skip если `KEPLER_INSTANCE.hotkey === null`.
- `setAutostartEnabled(...)` — silent no-op если `!instance.autorunEnabled`.
- `deviceId = "kepler-shell-${KEPLER_INSTANCE.slot}"` (sync identity per slot).

## Multi-agent worktree workflow

```powershell
# Завести второй dev-инстанс в worktree:
git worktree add ../kepler-dev-a -b dev-a main
Set-Location ../kepler-dev-a
Copy-Item shell/.env.local.example shell/.env.local
# раскомментировать: KEPLER_INSTANCE=dev-a
bun install
bun run --cwd shell dev
# → tray: Kepler [dev-a], %APPDATA%\Kepler-dev-a\, %APPDATA%\Kosmos-dev-a\

# Параллельно установленный prod Kepler.exe продолжает работать.
# Параллельно третий агент в ../kepler-dev-b с KEPLER_INSTANCE=dev-b.
```

### Известные ограничения мульти-dev

- **HMR для extension'ов** (Vite dev server, порты 5180-5183) — opt-in через `KEPLER_DEV_EXTENSIONS=1`. Одновременно может быть запущен только **один** worktree с HMR (порты hardcoded в `manifest.json::devPort`). Остальные dev-инстансы работают с prebuilt `dist/` extension'ов — это всё ещё дает HMR для shell main process, но не для Vue extension'ов. На практике одного агента с HMR хватает.
- **`globalShortcut F12`** (toggle DevTools в dev) — Windows route'ит accelerator одному фокусному окну. В multi-dev только первый зарегистрировавшийся инстанс получит F12; остальные пользуются Tray → DevTools или меняют код через redocking.
- **Storage на диске**: каждый dev-slot хранит **полную** копию ARK DB (extensions + backups + crashes). Один dev-slot = ~50-200 MB. Имей это в виду при настройке 3-4 worktree'ев.

## Отвергнутые альтернативы

- **Docker / devcontainer** — Electron на Windows + GPU + Mica acrylic в контейнере не работает корректно.
- **Side-by-side NSIS install** с другим `appId`/`productName` (как VS Code Insiders) — полезно для «установить стабильный dev build рядом с prod», но это про инсталлятор, не про активную разработку с HMR. Можно добавить отдельным `electron-builder` target позже.
- **VM/Hyper-V per agent** — слишком дорого по RAM и UX.

## Миграция существующих dev-настроек

До этого изменения dev писал в `%APPDATA%\Kepler\` (тот же что prod) — `kepler-shell-settings.json` (developerMode toggle, hotkey override). После — `%APPDATA%\Kepler-dev\`.

`instance.ts::migrateLegacyDevSettings()` при первом запуске slot=`dev` копирует `kepler-shell-settings.json` из старого места в новое. Outdated state (`post-update.flag`, autoupdater cache, Local Storage) не копируется — autoupdater в dev отключён, остальное Chromium регенерирует.

После одного запуска migration становится no-op.

## AC проверки

Минимальный smoke (ручной):

1. Запустить installed `Kepler.exe`. В tray появляется иконка «Kepler».
2. В этом же worktree (без `.env.local`): `bun run --cwd shell dev`. В tray появляется вторая иконка «Kepler [dev]». Оба окна доступны через свои hotkey: prod — `Alt+Space`, dev — `` Alt+` ``.
3. Создать заметку в dev Eden. Открыть prod Eden — заметки нет (разные `ark.db`).
4. Завести второй worktree с `KEPLER_INSTANCE=dev-a`. Три иконки в tray. Три отдельных `ark.db`.

## Запреты (forbidden)

- ❌ Хардкодить `"Kepler"` / `"Kosmos"` / `app.getPath('userData')` / `%APPDATA%/Kepler/...` в новом коде. Только через `resolveInstance()` (либо `keplerDataDir()` для ARK dataDir).
- ❌ Звать `applyInstanceToApp()` повторно или из renderer. Только один раз на старте main.ts.
- ❌ Вызывать `app.requestSingleInstanceLock()` до `applyInstanceToApp()`. Lock scope'ится по userData — порядок критичен.
- ❌ Использовать `process.execPath` для `setLoginItemSettings` без проверки `instance.autorunEnabled`. Dev `electron.exe` из node_modules в HKCU не нужен.
