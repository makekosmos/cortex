# Focus mode

Источник: `platform/desktop/electron/focus-*.ts` + `platform/native-services/kepler-focus-helper/` + `platform/native-services/kepler-focus-svc/` + `platform/runtime/src/focus.rs` + `platform/runtime/src/pomodoro_host.rs`.

## За 30 секунд

Focus mode — это подсистема Kepler, которая:

1. Показывает shell-owned **Start Focus Session** внутри текущего Shell (`platform/desktop/src/views/LauncherView.vue` + `platform/desktop/src/components/FocusCommandPanel.vue`): цель с `@` mention для Delphi-задачи, длительность и blocklist. Управление текущей сессией идёт отдельными Shell commands (`toggle/pause/resume/skip/complete`) и через focus widget.
2. Показывает плавающий always-on-top **focus widget** (Spotify-mini-style, 320×52) с countdown'ом текущей pomodoro-фазы и кнопками pause/resume/skip/stop. Виджет живёт пока Kepler shell открыт и тикает по wallclock anchor'у даже без открытой Focus Session панели.
3. Блокирует **distracting домены** через запись в `C:\Windows\System32\drivers\etc\hosts` между маркерами `# === kepler-focus BEGIN/END ===`. Применением занимаются короткоживущие elevated процессы (`Kosmos Helper.exe`, dev bin `kepler-focus-helper.exe`) или постоянный Windows-сервис (`Kosmos System Service`, dev bin `kepler-focus-svc`), который снимает UAC-промпт после первой установки.
4. Блокирует запуск выбранных приложений **из Kosmos launcher**: Focus Session textarea принимает `@`-mentions приложений из `app_index`, сохраняет `blocked_app_ids` в active state, а `LauncherView` отказывает в `app_index.launch` пока focus active. Это не OS-level process blocker.
5. Хранит **blocklist'ы** и **active state** в ARK (тип объекта `blocklist_obj` + `sync_kv` ключ `focus.active_state`) через модуль `platform/runtime/src/focus.rs`. Применение к hosts file делает **shell**, backend знает только state.

Pomodoro session при этом сама по себе живёт в backend'е (`pomodoro_host.rs`, persisted state) и переживает рестарт shell'а — focus widget гидратируется при старте через `pomodoro.get_state`.

::: warning Digital Cave ≠ focus-mode
[Digital Cave](/apps/digital-cave) — это **зарезервированное имя** будущего полноценного приложения-блокера (app-level blocks, hard mode, schedules). Текущий focus-mode покрывает только domain-block через hosts file + pomodoro widget UI. См. различия ниже.
:::

## Компоненты

| Файл / бинарь                                                                                           | Что делает                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `platform/desktop/electron/focus-session.ts`                                                            | Main-process orchestration для shell-owned Focus Session: отдаёт snapshot/tasks/blocklists по IPC, стартует/останавливает `pomodoro.*`, создаёт/закрывает `time_entry_obj`, применяет blocklist через `applyFocusBlock`.                                                                                                                                                                          |
| `platform/desktop/electron/focus-widget.ts`                                                             | Создаёт `BrowserWindow` 320×52, frameless, alwaysOnTop, transparent. Lazy create при первом `setFocusState({ active: true })`. Position персистится в `kepler-focus-widget-state.json`. Autonomous tick раз в секунду из main process по `phaseEndsAtMs` wallclock anchor'у. Подписан на backend pomodoro events.                                                                                 |
| `platform/desktop/electron/focus-block.ts`                                                              | `applyFocusBlock({ active, domains })` — модифицирует hosts file. Тройной fallback: pipe-to-service → auto-install service (один UAC) → direct helper spawn (если Kepler сам admin) → elevated helper через `Start-Process -Verb RunAs` (UAC per-call).                                                                                                                                           |
| `platform/desktop/electron/focus-service.ts`                                                            | TS-клиент для Kosmos System Service: CLI invocations (install/uninstall/start/stop/status, elevation через PowerShell `RunAs`) + named-pipe IPC `\\.\pipe\kosmos-system-service` с fallback на legacy `\\.\pipe\kepler-focus-svc`.                                                                                                                                                                |
| `platform/desktop/src/views/LauncherView.vue` + `platform/desktop/src/components/FocusCommandPanel.vue` | Vue-renderer Start Focus Session внутри content slot текущего Shell: header back остаётся в launcher surface, цель и Delphi task выбираются в одном поле через `@`, форма фокуса не открывает отдельный `BrowserWindow`.                                                                                                                                                                          |
| `platform/desktop/src/views/FocusWidgetView.vue`                                                        | Vue-renderer виджета. Получает state через `kepler:focus-widget:state` IPC, кнопки дёргают `pomodoro.{pause,resume,skip,stop}` через `invokeOperation`.                                                                                                                                                                                                                                           |
| `platform/native-services/kepler-focus-helper/`                                                         | Dev Rust binary; packaged как `Kosmos Helper.exe` с `requireAdministrator` manifest. Читает один JSON request со stdin (или `--input <file>` если spawned через `Start-Process -Verb RunAs`), выполняет op над hosts file, печатает JSON response, exit. Маркер-секция + idempotent add/remove + backup в `hosts.kepler-backup` (один раз).                                                       |
| `platform/native-services/kepler-focus-svc/`                                                            | Dev Rust service; packaged как `Kosmos System Service.exe`. Windows-сервис (LocalSystem, AutoStart). Listens на named pipe `\\.\pipe\kosmos-system-service` с SDDL `D:(A;;GA;;;AU)` (доступно authenticated user-mode процессам), переиспользует `kepler_focus_helper::hosts` и даёт privileged NTFS scan для File Search. Legacy service name `KeplerFocusSvc` остаётся supported для migration. |
| `platform/runtime/src/focus.rs`                                                                         | ARK operations: `focus.list_blocklists`, `focus.upsert_blocklist`, `focus.delete_blocklist`, `focus.get_active_state`, `focus.set_active_state`. Хранит **только state** (`blocklist_id`, `blocked_app_ids`), не трогает hosts file.                                                                                                                                                              |
| `platform/runtime/src/pomodoro_host.rs`                                                                 | Singleton `Session` в `Arc<Mutex<...>>`, tokio ticker раз в секунду. Persisted state в `<data_dir>/pomodoro-state.json` (переживает рестарт shell + backend). Broadcast `pomodoro_tick` / `pomodoro_phase_changed` / `pomodoro_finished` через WS всем подписчикам.                                                                                                                               |
| `platform/desktop/electron/pomodoro-notifier.ts`                                                        | OS-toast уведомления на phase_changed (work↔break) независимо от того, открыта ли Focus Session панель.                                                                                                                                                                                                                                                                                           |

## Поток данных

```mermaid
flowchart LR
  focus[Focus Session UI<br/>Shell content panel]
  shell[Kepler shell<br/>LauncherView + focus-session.ts]
  backend[kepler-backend<br/>focus.rs + pomodoro_host.rs]
  widget[Focus widget<br/>BrowserWindow]
  svc[Kosmos System Service<br/>Windows Service]
  helper[Kosmos Helper.exe<br/>elevated]
  hosts[(C:\\Windows\\System32\\<br/>drivers\\etc\\hosts)]
  notif[OS toast notifications]

  focus -- "kepler.focusSession.* IPC" --> shell
  shell -- "pomodoro.start / focus.set_active_state" --> backend
  backend -- "pomodoro_tick / phase_changed (WS)" --> shell
  shell -- "deriveFocusStateFromBackend()" --> widget
  shell -- "pomodoro_phase_changed" --> notif
  shell -- "applyFocusBlock(active, domains)" --> svc
  svc -. "pipe ENOENT / not running" .-> helper
  shell -- "fallback: spawn / RunAs" --> helper
  svc --> hosts
  helper --> hosts
  widget -- "pomodoro.{pause,resume,skip,stop}" --> backend
```

Ключевые инварианты:

- **Backend — source of truth для pomodoro lifecycle.** Все операции pause/resume/skip/stop проходят через `pomodoro.<op>` → backend → broadcast → подписчики. Виджет не дёргает `setFocusState` локально после кнопки; ждёт ответ backend'а.
- **Focus Session main process владеет side effects.** UI отправляет intent через `window.kepler.focusSession.*`; main process вызывает `pomodoro.*`, `focus.*`, `applyFocusBlock(...)` и ARK-safe `time_entry_obj` upsert.
- **Widget tick autonomous.** `phaseEndsAtMs` (wallclock unix-ms) — anchor; main process пересчитывает `remainingSec` раз в секунду даже без открытой Focus Session панели.

## IPC / wire endpoints

### Electron IPC (shell main ↔ renderer)

| Канал                                                        | Направление          | Назначение                                                                   |
| ------------------------------------------------------------ | -------------------- | ---------------------------------------------------------------------------- |
| `kepler:focus-session:open`                                  | renderer → main      | request открыть Focus Session внутри текущего Shell                          |
| `kepler:focus-session:open-shell`                            | main → launcher      | перевести `LauncherView` в режим Focus Session                               |
| `kepler:focus-session:{snapshot,list-tasks,list-blocklists}` | renderer → main      | hydrate формы Focus Session                                                  |
| `kepler:focus-session:{start,pause,resume,skip,stop}`        | renderer → main      | lifecycle Focus Session через backend `pomodoro.*` и shell side effects      |
| `kepler:focus-session:updated`                               | main → renderer      | refresh открытых Focus Session panels после backend/main-process изменений   |
| `kepler:focus-widget:set-state`                              | renderer → main      | compat/internal push `Partial<FocusState>`                                   |
| `kepler:focus-widget:get-state`                              | renderer → main      | initial hydrate в `FocusWidgetView`                                          |
| `kepler:focus-widget:state`                                  | main → renderer      | broadcast updated state виджету                                              |
| `kepler:focus-widget:hide`                                   | renderer → main      | спрятать виджет (не destroy)                                                 |
| `kepler:focus-widget:pomodoro:{pause,resume,skip,stop}`      | renderer → main      | проксируется в `invokeOperation("pomodoro.<op>")`                            |
| `kepler:focus-widget:stopwatch:stop`                         | renderer → main      | закрывает running `time_entry_obj` (source=manual) напрямую через ARK upsert |
| `kepler:focus:applied`                                       | main → all renderers | результат `applyFocusBlock` (Settings показывает status)                     |
| `kepler:focus-service:status-changed`                        | main → all renderers | service install/uninstall — UI рефрешит карточку                             |

### ARK operations (backend)

`focus.list_blocklists` / `focus.upsert_blocklist` / `focus.delete_blocklist` / `focus.get_active_state` / `focus.set_active_state` — см. `platform/runtime/src/focus.rs`. Shell-owned Focus Session после успешного `focus.set_active_state` дёргает `applyFocusBlock(...)`; extension-host сохраняет compat middleware для extension calls.

### Pipe protocol (`\\.\pipe\kosmos-system-service`)

JSON request → JSON response, single round-trip, pipe закрывается. См. `platform/native-services/kepler-focus-svc/src/protocol.rs`:

```text
{"op":"add","domains":["tiktok.com","twitter.com"]}
{"op":"remove","domains":["twitter.com"]}
{"op":"reset"}
{"op":"status"}
{"op":"ping"}
{"op":"ntfs_scan","root":"C:\\","exclude_noisy":true}
```

Response: `{ok, active_domains?, error?, pong?}`. Helper bin использует тот же shape минус `ping`.

## Hosts file дисциплина

- Модификации только между маркерами `# === kepler-focus BEGIN ===` / `# === kepler-focus END ===`. Всё что вне маркеров — read-only.
- Перед первой модификацией создаётся `hosts.kepler-backup` (sibling). На `reset` восстанавливаем backup; если backup отсутствует — просто срезаем managed-секцию.
- `add` идемпотентен: нормализация (lowercase, strip `www.`), дедуп, sorted. Для каждого домена пишутся обе записи `127.0.0.1 <domain>` и `127.0.0.1 www.<domain>`.
- Запись атомарная: `<hosts>.tmp` → `rename` поверх + verify readback. На mismatch — `HostsError::Verify`.
- Override для тестов: env `KEPLER_FOCUS_HOSTS_PATH` (используют unit-тесты в `hosts.rs` и `protocol.rs`).

## Запреты

См. [полный список](/agents/forbidden#focus-mode). Кратко:

- ❌ Прямые манипуляции `BrowserWindow` focus widget'а из extension'ов. Только через `kepler:focus-widget:*` IPC.
- ❌ Обход `pomodoro_host` для lifecycle pomodoro-сессии. Никаких прямых state-mutations из shell — только `invokeOperation("pomodoro.<op>")`.
- ❌ Прямые writes в hosts file из любого места кроме `Kosmos Helper.exe` / `Kosmos System Service.exe` (dev: `kepler-focus-helper` / `kepler-focus-svc`). Никаких inline `fs.writeFile("C:\\Windows\\...")` из shell или extension'ов.
- ❌ Запись вне маркерной секции в helper/svc — backup может не покрыть, юзер потеряет свои hosts entries.
- ❌ Destructive schema migration для `blocklist_obj` (см. [ARK objects](/concepts/ark-objects)).
- ❌ `setupFocusWidgetBackendSync` без последующего `teardownFocusWidgetBackendSync` при backend respawn — приведёт к двойной подписке после `resetArkClient`.

## Открытые вопросы / TODO

- OS-level app blocking (process_name / window title, запрет запуска вне Kosmos) — не реализован. Это часть будущего [Digital Cave](/apps/digital-cave). Текущий Focus mode блокирует только `app_index.launch` внутри Kosmos launcher.
- Hard mode (нельзя выключить до конца таймера) — не реализован.
- Расписания (например, «будни 9–13») — не реализованы.
- Логирование попыток обхода (`block_attempt_obj`) — не реализовано.
- Browser-extension путь для domain blocks (как альтернатива hosts file) — не рассматривался.

## Связанные документы

- [Digital Cave](/apps/digital-cave) — будущая полноценная app-блокировка.
- [Kepler](/apps/kepler) — shell-owned Focus Session и launcher command.
- [Command bus](/concepts/command-bus) — wire-format pomodoro events.
- [Extension host](/concepts/extension-host) — где живёт middleware `focus.set_active_state` → `applyFocusBlock`.
