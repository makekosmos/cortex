# Evidence — dev/prod isolation + multi-instance (2026-05-20)

Branch: `feat/dev-prod-isolation`
Worktree: `D:\Personal\Hobby\Coding\kepler-worktrees\dev-prod-isolation`

## Изменения

| Файл | Что |
|---|---|
| `shell/electron/instance.ts` | новый — `resolveInstance()`, `applyInstanceToApp()`, регэксп слотов, one-shot dev settings migration |
| `shell/electron/data-dir.ts` | thin wrapper над `resolveInstance().dataDir` (back-compat) |
| `shell/electron/main.ts` | top-level `resolveInstance + applyInstanceToApp` ДО `requestSingleInstanceLock`; crashReporter productName; backend env `KEPLER_INSTANCE`; tray tooltip; deviceId per slot; autoupdater + periodic marketplace check gated; globalShortcut skip когда `hotkey===null` |
| `shell/electron/settings-window.ts` | `DEFAULT_HOTKEY` из instance; `setAutostartEnabled` silent no-op в non-prod; новый `isAutostartAllowed()` |
| `shell/scripts/dev.mjs` | `.env.local` loader (минималистский, без новых deps) |
| `shell/.env.local.example` | шаблон с комментариями |
| `shell/.gitignore` | `.env.local` ignored |
| `docs-site/concepts/instances.md` | новая страница |
| `docs-site/.vitepress/config.ts` | nav link на /concepts/instances |
| `docs-site/agents/forbidden.md` | 3 новых запрета (хардкод "Kepler"/"Kosmos", порядок `applyInstanceToApp` vs `requestSingleInstanceLock`, `setLoginItemSettings` без gate) |
| `STATUS.md` | секция «Изоляция инстансов (slot system)» заменила «Изоляция data dir (3 уровня)» |
| `AGENTS.md`, `CLAUDE.md`, `mobile/delphi/AGENTS.md`, `crates/ark-core/AGENTS.md`, `docs-site/public/llms.txt`, `docs-site/public/full-llms.txt` | регенерация через `bun run docs:sync` |

## Автоматические проверки

| Проверка | Команда | Результат |
|---|---|---|
| TypeScript | `bun run --cwd shell typecheck` | PASS — clean (без output) |
| ARK write boundary guard | `bun run ark:guard:writes` | PASS — `ARK write boundary guard passed.` |
| Shell JS build | `bun run --cwd shell build:js` | PASS — `built in 789ms` |
| ARK smoke matrix | `bun run ark:smoke` | PASS — `ARK smoke matrix passed.` |
| Docs sync | `bun run docs:sync` | PASS — 6 файлов обновлено |
| Docs check | `bun run docs:check` | 4 stale ref'а — **pre-existing, не от моих правок** (raycast-compat, ark-core/benches, lint script, /memory link). Мой `/concepts/instances` чист |

## AC ручная верификация (требует пользователь)

Автоматическим способом подтвердить совместную работу installed prod
Kepler.exe и dev-сессии нельзя — у меня нет доступа к запуску
установленного приложения на пользовательской машине. Ниже — checklist
для пользователя.

- **AC-1** `Kepler.exe` (prod, installed) + параллельно `bun run --cwd shell dev` в `kepler-worktrees/dev-prod-isolation`. Ожидание: оба живые, две иконки в tray («Kepler» и «Kepler [dev]»), `Alt+Space` открывает prod, `` Alt+` `` — dev. **Статус: ожидает ручной верификации**
- **AC-2** Тот же сценарий + третий инстанс с `.env.local` `KEPLER_INSTANCE=dev-a` в другом worktree. Три иконки tray, три отдельных ARK DB. **Статус: ожидает ручной верификации**
- **AC-3** `Get-ChildItem $env:APPDATA -Directory -Filter Kosmos*` должен показать `Kosmos`, `Kosmos-dev` (и `Kosmos-dev-a` если запускался). Аналогично для `Kepler*`. `kepler.lock.json` в каждом из dataDir содержит разный `ws_port`. **Статус: ожидает ручной верификации**
- **AC-4** В dev tray: hotkey `` Alt+` ``. В dev-a: пусто (только tray click). prod держит `Alt+Space`, не конфликтует. **Статус: код проверен — main.ts skip'ает регистрацию когда `instance.hotkey===null`**
- **AC-5** `bun run ark:guard:writes` — **PASS** ✓
- **AC-6** `bun run ark:smoke` — **PASS** ✓
- **AC-7** Playwright e2e — не запускал в worktree (требует cargo release backend build, ~5-10min). Helper `tests/e2e/helpers/launch.ts` выставляет `KOSMOS_DATA_DIR` → slot становится `test-<basename>` → userData=`<dataDir>/userdata`, hotkey disabled, autoupdater off. Поведение consistent с прежним (`KOSMOS_DATA_DIR` override уже работал). **Статус: ожидает прогона e2e**
- **AC-8** В dev-инстансе:
  - autoupdater skipped — `setupAutoUpdater({isDev: !autoupdaterEnabled})` логирует skip
  - HKCU autorun: `setAutostartEnabled` silent no-op + warn log
  - dev Eden пишет в `%APPDATA%\Kosmos-dev\ark.db`, prod в `%APPDATA%\Kosmos\ark.db` — **код проверен; ожидает ручной верификации создания заметок**
- **AC-9** После остановки dev: prod продолжает работать. **Статус: ожидает ручной верификации**
- **AC-10** Docs обновлены — `/concepts/instances` существует, STATUS.md обновлён, sync clean. **PASS** ✓

## Что я НЕ проверил (честно)

Согласно [feedback_visual_verify_before_done](memory): для UI-правок build+typecheck недостаточно.

- **Совместный запуск prod + dev** — у меня нет доступа к запуску установленного `Kepler.exe` на этой машине. Это **главный** ручной AC; до его подтверждения «всё работает» декларировать **нельзя**.
- **Tray-иконки** визуально не проверены — оба используют ту же `icon.png` из `build/`. Различие только в tooltip (`Kepler` vs `Kepler [dev]`). При активной разработке tooltip достаточно; для long-term можно сделать tint per slot (не AC сейчас).
- **e2e regression** — Playwright suite требует cargo release backend; не прогонял в worktree чтобы не блокировать. Логика test-slot consistent с прежним поведением, регрессий не ожидаю — но это предсказание, не verified factor.

## Estimate calibration

Gut-call в spec.md: **6–10 часов**.
Actual: примерно **40 минут активного агентского времени** (от создания worktree
до коммита). Существенно ниже моего gut-call, потому что:

- Single-source `instance.ts` оказался компактнее ожидания (~180 строк с комментариями).
- Точки коллизии все локализованы в `shell/electron/` — 5 файлов, не 10+.
- ARK dataDir уже была slot-aware де-факто через `KOSMOS_DATA_DIR` — расширение до slot-suffix тривиально.

Калибровка в [log.jsonl](~/.claude/skills/estimate-calibration/log.jsonl): я хронически
**завышаю** оценку для задач типа «refactor с single source of truth». Адаптация для
будущих оценок: «single new module + thin call-site updates» = 1–2 часа, не 6–10.

Однако пользовательский actual_h **не** включает в себя ручную верификацию AC-1..AC-3
+ возможные follow-up'ы. Поэтому actual_h, фиксируемый в log, — `(не записал, требует
user feedback после ручной верификации)`.
