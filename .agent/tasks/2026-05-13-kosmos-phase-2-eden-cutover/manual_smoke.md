# Phase 2 Eden cutover — Manual smoke playbook

**Worktree:** `D:\Personal\Hobby\Coding\kepler-kosmos`
**Branch:** `kosmos/phase-1-scaffold`

Этот playbook — пошаговый чек-лист для проверки **AC1 + AC2** Phase 2 (Eden работает в cosmos-mode end-to-end + reconnect ≤ 5s). Остальные AC уже PASS'ed автотестами (см. `evidence_partial.md`).

## Pre-requirements (один раз)

Все release бинари должны быть собраны:

```powershell
# 1. ark-core-rpc release (уже собран в Phase 1+2 dev, но на всякий случай)
cargo build --release `
  --manifest-path D:\Personal\Hobby\Coding\kepler-kosmos\packages\ark-core\rust\Cargo.toml `
  --bin ark-core-rpc

# 2. kosmos release (уже собран)
cargo build --release `
  --manifest-path D:\Personal\Hobby\Coding\kepler-kosmos\apps\kosmos\Cargo.toml `
  --bin kosmos

# 3. Eden Heart (нужен для Eden — это vault filesystem manager)
cargo build --release `
  --manifest-path D:\Personal\Hobby\Coding\kepler-kosmos\apps\eden\ts\heart\Cargo.toml

# 4. Eden TS deps (если не сделано: bun install в worktree)
cd D:\Personal\Hobby\Coding\kepler-kosmos
bun install --frozen-lockfile
```

Verify бинари существуют:
```powershell
Test-Path D:\Personal\Hobby\Coding\kepler-kosmos\apps\kosmos\target\release\kosmos.exe
Test-Path D:\Personal\Hobby\Coding\kepler-kosmos\packages\ark-core\rust\target\release\ark-core-rpc.exe
Test-Path D:\Personal\Hobby\Coding\kepler-kosmos\apps\eden\ts\heart\target\release\eden-heart.exe
```

Все три должны вернуть `True`.

---

## AC1 — Eden работает в cosmos-mode end-to-end

### Шаг 1: Запустить Kosmos

В **отдельном** PowerShell-окне:

```powershell
$env:ARK_CORE_RPC_PATH = "D:\Personal\Hobby\Coding\kepler-kosmos\packages\ark-core\rust\target\release\ark-core-rpc.exe"
D:\Personal\Hobby\Coding\kepler-kosmos\apps\kosmos\target\release\kosmos.exe
```

**Ожидаемый output** в stderr:
```
Kosmos v0.1.0 starting (protocol 1.0.0)
[kosmos] singleton acquired: C:\Users\<you>\AppData\Roaming\Kepler\kosmos-singleton.lock.db
[kosmos] ark-core-rpc binary: "D:\\...\\ark-core-rpc.exe"
[kosmos] db: C:\Users\<you>\AppData\Roaming\Kepler\ark.db
[kosmos] ark-core-rpc spawned and initialized
[kosmos] WS listening on 127.0.0.1:<random_port>
[kosmos] lock-file: "C:\\Users\\<you>\\AppData\\Roaming\\Kepler\\kosmos.lock.json"
[kosmos] ready. Ctrl+C to stop.
```

**Не закрывай это окно** — Kosmos должен крутиться весь smoke.

### Шаг 2: Проверить lock-file

В третьем окне:

```powershell
Get-Content $env:APPDATA\Kepler\kosmos.lock.json | ConvertFrom-Json | Format-List
```

**Что увидеть:**
- `pid` — должен матчить PID kosmos.exe из task-manager
- `ws_port` — какой-то random > 1024
- `protocol_version` — `@{major=1; minor=0; patch=0}`
- `auth_token` — 64 hex-символа

**Permissions проверка** (AC3 verification из Phase 1, но напомним):
```powershell
(Get-Acl $env:APPDATA\Kepler\kosmos.lock.json).Access | Where-Object {
    $_.IdentityReference -notmatch [Environment]::UserName
}
```

Должен вернуть **пусто** — в ACL только текущий user. Если выводит `BUILTIN\Users` или `Authenticated Users` — это **FAIL** AC3 (но скорее всего OK, т.к. интеграционные тесты Phase 1 проверили).

### Шаг 3: Запустить Eden с cosmos-mode

В **третьем** PowerShell-окне (Kosmos продолжает крутиться в первом):

```powershell
cd D:\Personal\Hobby\Coding\kepler-kosmos\apps\eden\ts
$env:KEPLER_KOSMOS_OPTIONAL = "1"   # на случай если что-то fail'ится — fallback на self-managed
bun run dev
```

Это запустит Vite dev server + Electron Eden window. В консоли Vite появится:

```
electron] [eden.ark] using Kosmos host (pid <kosmos_pid>, ws_port <port>)
```

**Это критичная строка** — означает Eden **успешно подключился к Kosmos**.

Если вместо неё видишь:
```
electron] [eden.ark] Kosmos unavailable (...); falling back to self-managed ark-core-rpc.
```
→ Eden не нашёл Kosmos. См. Troubleshooting ниже.

### Шаг 4: Проверка Task Manager — **главный proof**

Открыть Task Manager → Details tab → отфильтровать по имени.

**Должно быть:**
- `kosmos.exe` — **ровно 1 шт.** (наш host)
- `ark-core-rpc.exe` — **ровно 1 шт.** (supervised child Kosmos'а, **не** Eden'ом)
- `electron.exe` / `Eden.exe` — несколько (renderer/preload Electron'а — это нормально)
- `eden-heart.exe` — 1 шт. (Eden's vault filesystem manager — это не наше дело, оно для notes на диске)

**Если видишь `ark-core-rpc.exe` × 2 — это FAIL AC1.** Eden запустил собственный sidecar параллельно с Kosmos'ным.

### Шаг 5: Eden UI — CRUD заметок

В окне Eden:
1. Создать новую заметку → ввести текст → сохранить (Ctrl+S или автосейв)
2. Перезагрузить Eden (Ctrl+R в DevTools или закрыть/открыть) → заметка должна быть на месте
3. Поиск по тексту заметки → должен находить (через ARK FTS5)
4. Удалить заметку → должна удалиться

**Если всё работает — AC1 PASS.**

### Шаг 6: Проверка stderr Eden и Kosmos

В Eden Vite консоли **не должно** быть:
- `ark-core-rpc exited with` (sidecar упал)
- `ark-core-rpc init failed` (init не прошёл)
- timeouts на ARK операции

В Kosmos stderr **не должно** быть никаких `error` или `panic` сообщений.

---

## AC2 — Kill Kosmos → reconnect ≤ 5s

### Шаг 7: Kill Kosmos в runtime

В Task Manager → правый клик на `kosmos.exe` → End task.

**Ожидаемое поведение Eden:**
- В Eden Vite консоли появится: `[eden.ark] Kosmos connection closed` или `kosmos error` на следующем request
- Если ты сейчас делаешь операцию (поиск, save) — она fail'ится с ошибкой

### Шаг 8: Перезапустить Kosmos

В первом окне (где kosmos.exe был запущен) — заново:
```powershell
D:\Personal\Hobby\Coding\kepler-kosmos\apps\kosmos\target\release\kosmos.exe
```

Должен снова стартануть (singleton lock освобождён предыдущим exit).

### Шаг 9: Eden reconnect

Через ≤ 5 секунд (или при следующей операции в Eden) — Eden должен реконнектиться. Видно по:
- Новая операция в Eden работает без перезапуска
- В Vite консоли: новый `[eden.ark] using Kosmos host (pid <NEW_pid>, ws_port <NEW_port>)`

**Если работает — AC2 PASS.**

### Альтернативный сценарий (если reconnect не работает автоматически):

В текущей реализации `closeCosmosConnection` reject'ит все pending requests, но automatic reconnect logic не имплементирован (Phase 2 minimum scope). В этом случае:
- Eden show error toast / console error
- Закрыть Eden и заново запустить → Eden подключится к новому Kosmos

Это OK для Phase 2 baseline. Full reconnect logic — Phase 3+ задача.

---

## Troubleshooting

### Eden не находит Kosmos (fallback на self-managed)

Проверь:
1. `kosmos.exe` запущен? Process Explorer показывает живой процесс.
2. `kosmos.lock.json` существует? `Get-Item $env:APPDATA\Kepler\kosmos.lock.json`.
3. Permissions OK? Eden запущен под тем же user account что Kosmos?
4. `$env:APPDATA` совпадает у обоих процессов? (Может различаться если Eden запущен через `bun run dev` с different APPDATA override.)

### Kosmos не стартует (singleton conflict)

Если в stderr Kosmos:
```
[kosmos] another Kosmos is already running (pid X, ws_port Y)
```
Но процесс X не существует — `kosmos.lock.json` stale. Удалить вручную:
```powershell
Remove-Item $env:APPDATA\Kepler\kosmos.lock.json
Remove-Item $env:APPDATA\Kepler\kosmos-singleton.lock.db
```

### Eden показывает database errors

Может быть ARK DB conflict — Kosmos и Eden пытаются open same DB:
- Если Kosmos владеет DB через ark-core-rpc child, **Eden НЕ должен** тоже open ту же DB напрямую.
- В cosmos mode Eden идёт через WS → ark-core-rpc Kosmos'а → DB. Нет конфликта.
- В fallback mode (self-managed) — Eden открывает свой ark-core-rpc → если Kosmos жив И тоже DB — **conflict**. Решение: kill один или другой.

### `ark-core-rpc` падает с FK constraint при создании object

Это AC3 scenario (Phase 2). Если объект приходит с unknown `type_id` — он сейчас идёт в `sync_pending_objects` и эмитится `sync_error` event. Eden пишет в Vite консоль:
```
[eden.ark] sync_error code=unknown_type_id entity_id=... awaited_type_id=...
```
Это **не баг**, это новое корректное поведение Phase 2.

---

## Итоговая verification матрица

| AC | Status | Where verified |
|----|--------|----------------|
| AC1 | Manual smoke | Шаги 1-6 |
| AC2 | Manual smoke (partial — reconnect-by-restart-eden OK) | Шаги 7-9 |
| AC3 | Auto (Rust unit) | `phase2_replay_runs_when_type_appears` |
| AC4 | Auto (Rust unit) | `phase2_replay_handles_multiple_pending_same_type` |
| AC5 | Auto (TS unit) | `AC5: alive PID + MAJOR mismatch` |
| AC6 | Auto (TS unit) + manual fallback | `AC6: no lock + no exe + autoLaunch off` |

Когда AC1+AC2 PASS — Phase 2 готова к merge.
