# Focus mode запреты

::: tip Узкий файл
Читайте только когда задача касается этой области. Полный legacy reference: `docs-site/agents/forbidden.md`.
:::

### Focus mode

См. [Focus mode](/concepts/focus-mode).

- ❌ Прямые манипуляции `BrowserWindow` focus widget'а (show/hide/move/destroy) из extension'ов или из кода вне `platform/desktop/electron/focus-widget.ts`. Только через IPC `kepler:focus-widget:*` (`set-state` / `get-state` / `hide`).
- ❌ Обход `pomodoro_host` для lifecycle pomodoro-сессии. Кнопки виджета (pause/resume/skip/stop) дёргают **только** `invokeOperation("pomodoro.<op>")` через backend — никаких прямых `setFocusState` локально после клика. Backend — source of truth, его broadcast обновит widget.
- ❌ Прямые writes в `C:\Windows\System32\drivers\etc\hosts` из любого места кроме `Kosmos Helper.exe` / `Kosmos System Service.exe` (dev-бинарники всё ещё называются `kepler-focus-helper` / `kepler-focus-svc`). Никаких inline `fs.writeFile` или `child_process` поверх hosts из shell / extension'ов / `platform/runtime/`.
- ❌ Запись вне маркерной секции (`# === kepler-focus BEGIN/END ===`) в helper / svc. Backup создаётся **один раз** при первой модификации — если перезаписать вне маркеров, юзерские hosts entries потеряются навсегда.
- ❌ Destructive ALTER / DROP для `blocklist_obj` или ключа `focus.active_state` в `sync_kv`. Только additive миграции (см. [ARK objects](/concepts/ark-objects)).
- ❌ `setupFocusWidgetBackendSync` без последующего `teardownFocusWidgetBackendSync` при backend respawn / `resetArkClient`. Двойная подписка → каждый pomodoro event handled дважды.
- ❌ Применение блокировки (hosts write) из `platform/runtime/src/focus.rs`. Модуль хранит **только state** в ARK; применение делает shell через `applyFocusBlock` middleware в `extension-host.ts`. Никакого privileged кода в backend.
- ❌ Trust'нуть pipe ответу без safety timeout. `sendViaPipe` всегда финиширует за 3s даже при mute pipe.
- ❌ Удалять `requireAdministrator` manifest у `Kosmos Helper.exe` / dev-бинаря `kepler-focus-helper.exe`. Без него helper стартует non-elevated и hosts write молча падает с access denied.
- ❌ Расширять SDDL `Kosmos System Service` pipe'а за пределы `D:(A;;GA;;;AU)` (Authenticated Users). NULL-DACL = network exposure, не нужно.
- ❌ Автоматически re-prompt'ить UAC для auto-install `Kosmos System Service` после того, как юзер отказался. `autoInstallAttemptedThisSession` (session-scope) + `setFocusServiceAutoInstallDeclined` (persisted) гарантируют один промпт максимум.
