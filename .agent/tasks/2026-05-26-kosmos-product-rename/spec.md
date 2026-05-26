# Kosmos Product Rename Migration

Дата старта: 2026-05-26.

## Контекст

Kepler сейчас является наружным именем Electron launcher'а, но продуктовая
модель упрощается до одного внешнего бренда: Kosmos. Пользователь должен видеть
понятные процессы и ярлыки, а существующие установки должны обновляться без
ручного удаления и повторной установки.

## Scope

В scope этой задачи:

- Наружное имя Electron-приложения: installer, shortcut, productName, window
  title, appId и автозапуск переходят с Kepler на Kosmos.
- Миграция существующего Electron userData из `%APPDATA%/Kepler` в
  `%APPDATA%/Kosmos App`, чтобы настройки shell'а и Chromium state не терялись.
- Autostart остаётся включённым у существующего пользователя после обновления:
  новый код читает/пишет `Kosmos` entry и чистит legacy `Kepler` entry.
- Service/helper получают понятные display names для нового UI/process story,
  без ломания уже установленных service instances.
- Документация фиксирует staged rename: внешний продукт — Kosmos, внутренние
  кодовые namespaces `kepler:*` и пути могут оставаться до отдельного cleanup.

Не в scope:

- Слияние `kepler-backend.exe` и `ark-core-rpc.exe` в один процесс. Это отдельная
  архитектурная задача.
- Массовый rename исходных директорий `shell/`, `services/kepler-backend`,
  IPC namespace `kepler:*`, package name `kepler-shell`.
- Переименование ARK lock file `kepler.lock.json` или wire protocol.

## Acceptance Criteria

**AC1. Product naming.** `shell/package.json` production build создаёт
`Kosmos.exe` / `Kosmos Setup X.Y.Z.exe`, ярлыки называются `Kosmos`, appId
production slot = `com.kazui.kosmos`, а user-facing window titles используют
Kosmos.

**AC2. Existing user migration.** При первом запуске production slot новый
`userData` путь `%APPDATA%/Kosmos App` получает settings/Chromium state из
legacy `%APPDATA%/Kepler`, если новый путь ещё пустой. ARK data dir остаётся
`%APPDATA%/Kosmos` и не переносится.

**AC3. Autostart migration.** Production autostart helper читает новый
`Kosmos` login item, сохраняет включённый legacy `Kepler` autostart как enabled,
и при `setAutostartEnabled` удаляет legacy `Kepler` entry best-effort.

**AC4. Service naming compatibility.** Windows service CLI использует новый
service name/display name для новых installs, но `status/start/stop/uninstall`
понимают legacy `KeplerFocusSvc`, чтобы существующие установки можно было
обслужить без ручного удаления.

**AC5. Verification.** Проходят `bun run --cwd shell typecheck` и targeted Rust
build для `kepler-focus-svc` / `kepler-focus-helper` / `kepler-backend`.

**AC6. Docs.** `STATUS.md` и docs-site отражают staged migration и явно
предупреждают, что one-process `Kosmos Runtime.exe` не входит в эту задачу.
