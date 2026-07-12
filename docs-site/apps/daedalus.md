# Daedalus — управление Codex

Daedalus — first-party Vue extension (`products/daedalus`) для управления долгоживущими Codex-сессиями внутри Kosmos. Desktop UI работает в extension `BrowserWindow`, но процессы агентов принадлежат `kepler-backend`: закрытие окна не завершает сессию.

## Runtime boundary

```text
Daedalus Vue/Pinia
  → @kosmos/ark client.agents
  → extension ARK IPC + authenticated Kepler WebSocket
  → platform/runtime/src/agents
  → Codex app-server JSONL + отдельный Git worktree
```

- Новый daemon или extension-sidecar не создаётся.
- На каждую активную сессию запускается отдельный `codex app-server --stdio`.
- TUI output не парсится; integration adapter принимает JSONL notifications и server requests.
- Codex CLI продолжает владеть credentials и `config.toml`.
- Неизвестные события логируются и не ломают сессию.
- При штатном shutdown backend отправляет каждому session actor команду завершения и ждёт остановки дочернего app-server.

## Storage и recovery

Daedalus не пишет в ARK sync tables. Projects, sessions, normalized timeline, approvals и Git metadata хранятся в:

```text
<KOSMOS_DATA_DIR>/extensions-data/daedalus/daedalus.db
```

Worktree создаются в `extensions-data/daedalus/worktrees/<repo-hash>/<session-id>`. Branch создаётся от текущего `HEAD`; dirty изменения основного checkout намеренно не копируются. Archive скрывает сессию, но не удаляет worktree. Отдельное подтверждаемое удаление доступно только для archived, inactive и clean worktree и не использует force.

При первом `agents.*` запросе runtime лениво открывает persisted state и возобновляет активные Codex threads. Renderer подписывается до snapshot/hydrate; `agents.snapshot(afterSeq)` возвращает актуальное состояние и доступный backlog, затем идут flat `agents_event` с persisted монотонным `seq`.

## Security

Manifest запрашивает только `agents.read`, `agents.control` и `dialogs.directory`. Directory picker возвращает выбранный путь, но не предоставляет renderer произвольный filesystem API. Запуск редактора ограничен allowlist `code`/`cursor`/`windsurf`/Explorer, а Changes не читает symlink targets за пределами worktree. Режим `full-access` требует отдельного подтверждения в UI.

## Deferred Android

Android-клиент будет отдельным Kotlin/Jetpack Compose приложением. Между desktop и mobile будет разделяться `agents.*` control protocol, а не Vue UI. Iroh/relay и текущий `iroh-spike` в desktop MVP не меняются.
