# Dev/prod полная изоляция Kepler + multi-instance (multi-agent worktree)

Дата: 2026-05-20
Branch: `feat/dev-prod-isolation`
Worktree: `D:\Personal\Hobby\Coding\kepler-worktrees\dev-prod-isolation`

## Проблема

Пользователь параллельно **разрабатывает** Kepler и **пользуется** prod-сборкой как личным софтом. Сейчас:

- prod Kepler.exe и `bun run --cwd shell dev` **не могут работать одновременно**: `app.requestSingleInstanceLock()` (shell/electron/main.ts:143) — общий по `appId=com.kazui.kepler`. Первый инстанс держит lock, второй редиректит focus и сразу exit.
- Electron `userData` определяется через `app.getName()` (= `productName=Kepler` в обоих случаях) → prod и dev пишут в один `%APPDATA%\Kepler\`:
  - `kepler-shell-settings.json` (developer mode, autorun, streamer mode и т.п.)
  - `post-update.flag` (autoupdater state machine)
  - window state, GPU cache, IndexedDB, Local Storage
- `globalShortcut("Ctrl+Shift+K")` — оба пытаются захватить один hotkey, второй молча не зарегистрируется.
- Tray-иконки обе подписаны `Kepler` — визуально не различить.
- HKCU autorun: dev не должен трогать.
- autoupdater в dev не должен пуллить релизы.

Что **уже** разделено и работает (не трогаем):
- `keplerDataDir()` в `shell/electron/data-dir.ts` — ARK data: prod `%APPDATA%\Kosmos\`, dev `%APPDATA%\Kosmos-dev\`, test `KOSMOS_DATA_DIR`. backend наследует через env.
- `kepler.lock.json` (backend WS port + bearer token) лежит **внутри** dataDir → backend port уже не конфликтует между prod и dev.

Дополнительно: пользователь хочет запускать **несколько dev-инстансов параллельно** в разных git worktree'ах для multi-agent разработки.

## Решение: instance slot + Electron side-by-side

Каждой сборке/worktree присваиваем уникальный `slot` (string, kebab-case). На его основе детерминированно derive'ятся **все** dimensions, что могут конфликтовать. Паттерн — как VS Code Stable / Insiders / Exploration: разный `productName`, разный `appId`, разный `userData`. Расширяем до N слотов для multi-agent.

### Слоты (соглашение)

| slot | trigger | Electron userData | ARK dataDir | productName | hotkey | autoupdater | autorun |
|---|---|---|---|---|---|---|---|
| `prod` (default) | installed Kepler.exe | `%APPDATA%\Kepler\` | `%APPDATA%\Kosmos\` | Kepler | Ctrl+Shift+K | on | разрешён |
| `dev` | `VITE_DEV_SERVER_URL` set, нет `KEPLER_INSTANCE` | `%APPDATA%\Kepler-dev\` | `%APPDATA%\Kosmos-dev\` | Kepler [dev] | Ctrl+Shift+Alt+K | off | запрещён |
| `dev-<x>` | `KEPLER_INSTANCE=dev-<x>` (per-worktree `.env.local`) | `%APPDATA%\Kepler-dev-<x>\` | `%APPDATA%\Kosmos-dev-<x>\` | Kepler [dev:<x>] | disabled | off | запрещён |
| `test-<slug>` | `KOSMOS_DATA_DIR` set (Playwright) | (под `KOSMOS_DATA_DIR/userdata/`) | `KOSMOS_DATA_DIR` (absolute) | Kepler [test] | disabled | off | запрещён |

`<x>` — произвольный slug `[a-z0-9-]+`. Соглашение: `dev-a`, `dev-b`, `dev-eden`, `dev-issue-42`.

### Resolution priority

1. `KEPLER_INSTANCE` env (explicit, override всего) — валидируется regex `^(prod|dev|dev-[a-z0-9-]+|test-[a-z0-9-]+)$`.
2. Иначе если `KOSMOS_DATA_DIR` set → `test-<basename>`.
3. Иначе если `VITE_DEV_SERVER_URL` set → `dev`.
4. Иначе → `prod`.

### Изменения в коде

1. **Новый `shell/electron/instance.ts`** — single source of truth:
   ```ts
   export interface Instance {
     slot: string;          // "prod" | "dev" | "dev-<x>" | "test-<x>"
     kind: "prod" | "dev" | "test";
     userDataDir: string;   // absolute path
     dataDir: string;       // absolute path (ARK)
     productName: string;
     appId: string;         // com.kazui.kepler[.<slot>]  (prod = com.kazui.kepler)
     hotkey: string | null; // null = disabled
     autoupdaterEnabled: boolean;
     autorunEnabled: boolean;
   }
   export function resolveInstance(): Instance;
   ```
   Должен вызываться **до** любого другого Electron API, кроме `app.getPath('appData')`.

2. **`shell/electron/main.ts`** — в самом начале `bootstrap()` (до `requestSingleInstanceLock`):
   ```ts
   const instance = resolveInstance();
   app.setName(instance.productName);
   app.setPath('userData', instance.userDataDir);
   ```
   Это разделит singleInstanceLock scope, window state, GPU cache.
   - `requestSingleInstanceLock` остаётся unchanged — он теперь scope'ится по новому userData.
   - `globalShortcut.register(instance.hotkey, …)` — skip если `null`.
   - `tray.setToolTip(instance.productName)`.
   - В `deviceId` строке (`main.ts:631`) использовать `instance.slot` вместо slice'а от userData чтобы сохранить human-readable id.

3. **`shell/electron/data-dir.ts`** — `keplerDataDir()` становится тонкой обёрткой `() => resolveInstance().dataDir`. Старая логика по `KOSMOS_DATA_DIR` / `VITE_DEV_SERVER_URL` переезжает внутрь `resolveInstance()`. Обратная совместимость на API сохраняется.

4. **`shell/electron/autoupdater-host.ts`** — early return в `installAutoUpdater()` если `!instance.autoupdaterEnabled`. UI кнопка «Проверить обновления» в Settings показывает inline-hint «недоступно в dev» вместо ошибки.

5. **`shell/electron/settings-window.ts`** — autorun toggle disabled (greyed out + tooltip) если `!instance.autorunEnabled`. HKCU не трогается из dev/test.

6. **`shell/electron/main.ts` backend spawn** — пробрасывает `KOSMOS_DATA_DIR=instance.dataDir` и `KEPLER_INSTANCE=instance.slot` в child env. Backend уже умеет читать `KOSMOS_DATA_DIR`; `KEPLER_INSTANCE` нужен для tagging crash log'ов (отделить prod crash'и от dev в одной поддиректории `<dataDir>/crashes/`).

7. **`shell/scripts/dev.mjs`** — загружает `<shell>/.env.local` (если есть) через простой parser (без новых deps), пробрасывает `KEPLER_INSTANCE` в child env. Без `.env.local` поведение прежнее (`dev` slot).

8. **`shell/.env.local.example`** (новый файл, in tree) — шаблон:
   ```
   # Раскомментировать чтобы запустить отдельный dev-инстанс
   # (не конфликтует с prod Kepler и другими dev-инстансами)
   # KEPLER_INSTANCE=dev-a
   ```

9. **`shell/.gitignore`** — добавить `.env.local`.

10. **`shell/electron/extension-host.ts`** + **`shell/electron/extension-installer.ts`** — extension install dir уже под `keplerDataDir()/extensions/`, дополнительных правок не требует. Sanity-check audit.

11. **Tray accent (опционально)** — для dev-* слотов рисуем монохромный overlay-точку (синий для `dev`, оранжевый для `dev-*`) на trayicon. Optional — если успеем; иначе только tooltip.

### Документация

- `docs-site/concepts/instances.md` (новый) — описание slot-системы, как завести dev worktree, таблица соглашений.
- Обновить `docs-site/concepts/system-requirements.md` секцию «Изоляция data dir» (3 уровня → N).
- Обновить корневой `STATUS.md` (раздел «Изоляция data dir»).
- `bun run docs:sync` → CLAUDE.md / AGENTS.md регенерируются автоматически.
- `docs-site/agents/forbidden.md` — добавить пункт: «Хардкодить `Kepler`/`Kosmos` без instance suffix в новом коде — только через `resolveInstance()`».

### Multi-agent worktree workflow (для пользователя)

```powershell
# Завести второй dev-инстанс в worktree:
git worktree add ../kepler-dev-a -b dev-a main
cd ../kepler-dev-a
Copy-Item shell/.env.local.example shell/.env.local
# отредактировать: KEPLER_INSTANCE=dev-a
bun install
bun run --cwd shell dev
# → tray «Kepler [dev:a]», %APPDATA%\Kepler-dev-a\, %APPDATA%\Kosmos-dev-a\

# Параллельно установленный prod Kepler.exe продолжает работать со своими данными.
# Параллельно третий агент в ../kepler-dev-b с KEPLER_INSTANCE=dev-b — тоже не конфликтует.
```

## Acceptance Criteria

- **AC-1** Prod Kepler.exe (`%LOCALAPPDATA%\Programs\Kepler\Kepler.exe`) запущен. Параллельно стартует `bun run --cwd shell dev` в worktree. **Оба** окна доступны, оба отображены в tray под разными именами (`Kepler` и `Kepler [dev]`), оба видят свои данные (создание заметки в dev не появляется в prod и наоборот).
- **AC-2** Те же действия с третьим инстансом: `KEPLER_INSTANCE=dev-a bun run --cwd shell dev` в worktree `kepler-dev-a`. Три tray-иконки одновременно, три раздельных ARK DB.
- **AC-3** Сетевой port'ы backend'ов не конфликтуют: `Get-NetTCPConnection -OwningProcess <pid>` для каждого инстанса показывает разные порты; `kepler.lock.json` в трёх разных dirs.
- **AC-4** `globalShortcut Ctrl+Shift+K` обслуживается **только** prod. В dev/dev-a hotkey либо `Ctrl+Shift+Alt+K`, либо disabled (см. таблицу). Регистрация не выбрасывает ошибок.
- **AC-5** `bun run ark:guard:writes` — clean.
- **AC-6** `bun run ark:smoke` — PASS.
- **AC-7** Playwright e2e (`tests/e2e/`) — все 13 specs PASS. Test slot (`test-<slug>`) работает через `KOSMOS_DATA_DIR` override как раньше; userData test-инстансов под `<KOSMOS_DATA_DIR>/userdata/`.
- **AC-8** В dev-инстансе:
  - autoupdater **не** пуллит релиз (логируется skip + UI-hint).
  - HKCU autorun toggle disabled (tooltip).
  - Создание заметки в Eden / задачи в Delphi → запись в `%APPDATA%\Kosmos-dev\ark.db`, **не** в `%APPDATA%\Kosmos\ark.db` (проверить sqlite3 dump'ом).
- **AC-9** После остановки dev-инстанса prod продолжает работать без сайд-эффектов (никакие dev файлы не перетёрты в `%APPDATA%\Kepler\`).
- **AC-10** Документация обновлена: `docs-site/concepts/instances.md` существует, `STATUS.md` отражает новую систему, `bun run docs:sync` + `bun run docs:check` clean.

## Out of scope

- Side-by-side **установленный** dev-build с отдельной иконкой в Start Menu (как VS Code Insiders.exe). Полезно, но отдельная задача — `electron-builder` target + другой `appId`/`productName` для NSIS. Текущая задача — про активную разработку с HMR.
- Docker / devcontainer / VM — для Electron на Windows непригодно (GPU, Mica acrylic).
- Slot-aware sync (sync между dev-a и prod) — НЕ нужно: дев-инстанс должен быть изолирован полностью, sync относится к peers, не к локальным slot'ам.
- Tray accent overlay per slot — nice-to-have, ниже AC.

## Risks

- **Existing user data**: после рефакторинга `keplerDataDir` остаётся `%APPDATA%\Kosmos\` для prod — без миграции. dev пользователи могут потерять `%APPDATA%\Kosmos-dev\`? Нет — путь не меняется для slot `dev`. Slot `dev-<x>` — новый, пустой.
- **Existing Electron userData**: до этого изменения dev писал в `%APPDATA%\Kepler\` (тот же что prod). После — `%APPDATA%\Kepler-dev\`. dev-окно потеряет window state и developer-mode toggle при первом запуске после merge. Это OK: developer mode легко выставить заново; window state — cosmetics. Добавить одноразовый migration в `instance.ts`: если slot=`dev` и `%APPDATA%\Kepler-dev\` пуст, а `%APPDATA%\Kepler\kepler-shell-settings.json` существует — скопировать settings.json (не trogать post-update.flag и autoupdater state). После одного запуска migration становится no-op.
- **`requestSingleInstanceLock` semantics**: Electron docs гарантируют, что lock scope'ится по userData. Проверить руками: запустить два инстанса с разным userData — оба живые. Если нет — fallback на explicit `appId` в `app.setAppUserModelId()`.
- **Backend `KOSMOS_DATA_DIR` override** уже работает. Sanity-check: smoke test не должен лезть в чужой data dir при параллельных инстансах.

## Estimate

Перед стартом — обращусь к `estimate-calibration` skill, прочитаю log.jsonl, запишу prediction. Gut-call сейчас: **6–10 часов** работы (TS + 1 новый module + минимальные точечные правки в 5 файлах + docs + 2 manual smoke прогона с измерением AC-1..AC-3 + e2e регрессия). Калибровку зафиксирую в evidence.md.
