# Что такое Kosmos

## Идея в одном абзаце

Kosmos — это **monorepo для личного софта одного человека**. В его центре общий рантайм данных **ARK** (Rust + SQLite + sync), а вокруг него — сфокусированные приложения: дневник, задачи, игровая библиотека, аналитика. Любая «штука пользователя» (заметка, задача, игра, сессия использования компьютера) — это **объект ARK**. Приложения — это разные UI-оболочки и интеграции поверх одной общей базы данных.

Главное архитектурное обещание: данные **живут на устройстве**, синхронизация (LAN или relay) — это бонус сверху, а не основа работы. Приложение остаётся полностью работоспособным без интернета и без облака.

## Зачем монорепо

- **Один контракт хранения.** ARK владеет схемой и форматом синхронизации. Приложения не изобретают каждый раз своё.
- **Один SDK.** Все Electron-приложения говорят с ARK через `@kosmos/ark`. Один тип ошибок, один API, одно место для эволюции.
- **Общий UI.** Sidebar, Titlebar, DesktopChrome, CommandPalette берутся из `@kosmos/visuals` и выглядят одинаково в Eden / Delphi / Arrancador / Dashboard.
- **Один процесс правок.** Substantial-задачи проходят через [proof loop](/concepts/proof-loop), правила едины для всех приложений.

## Требования

| Что            | Версия                                 | Зачем                                                                     |
| -------------- | -------------------------------------- | ------------------------------------------------------------------------- |
| **Bun**        | `1.3.5+`                               | пакет-менеджер, runner всех скриптов                                      |
| **Rust**       | stable + `cargo`                       | сборка `ark-core`, `kepler-backend`, `ark-relay-server`, `kepler-watcher` |
| **Node**       | 20+                                    | окружение для VitePress, native-зависимости Electron                      |
| **PowerShell** | 7+ (Windows)                           | большинство smoke-команд написаны под PS                                  |
| **Git**        | любая                                  | репозиторий                                                               |
| Платформа      | Windows 10/11 (основная), macOS, Linux | Electron-приложения проверяются преимущественно на Windows                |

## Первый запуск

```powershell
git clone <repo> kosmos
cd kosmos
bun install
```

После установки доступны команды верхнего уровня:

```powershell
bun run ark:guard:writes   # проверяет, что app services не пишут напрямую в ARK SQLite
bun run ark:smoke          # smoke matrix (см. Reference → Smoke-матрица)
bun run docs:dev           # запустить этот сайт локально на http://localhost:5173
bun run docs:build         # собрать статический сайт в docs-site/.vitepress/dist
```

Полный список — на странице [«Команды и скрипты»](/reference/commands).

## Сборка ARK-рантайма

ARK — это Rust-крейт + бинарь sidecar'а. Большинство пути сборки спрятаны за `cargo build --workspace`, но если нужно вручную:

```powershell
cargo build --manifest-path crates\ark-core\rust\Cargo.toml --bin ark-core-rpc
cargo test  --manifest-path crates\ark-core\rust\Cargo.toml
bun run --cwd packages/ark typecheck
```

Бинарь `ark-core-rpc` — канонический sidecar. Запускается из `services/kepler-backend` как child process; общается через stdin/stdout newline-delimited JSON. См. [ark-core](/packages/ark-core) и [Синхронизация](/concepts/sync).

## Запуск Kepler shell (главный путь)

```powershell
cd shell
bun run build:backend:dev   # cargo build (debug) services/kepler-backend
bun run dev                 # backend + extensions + Vite + Electron
```

`Ctrl+Shift+K` глобально откроет launcher. Tray-иконка появится в трее. Открой extension через launcher (Dashboard, Horologion, Delphi, Arrancador).

## Дальше

- [Структура репозитория](/guide/layout) — что где лежит.
- [Стек и инструменты](/guide/tooling) — Bun, Cargo, oxc (oxlint/oxfmt), Playwright и пр.
- [Рабочий процесс](/guide/workflow) — как правильно делать изменения.
- [Архитектура](/concepts/architecture) — как куски связаны.
