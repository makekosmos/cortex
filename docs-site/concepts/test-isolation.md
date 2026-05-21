# Изоляция тестовых БД

## Правило

::: danger Жёстко
Все **тесты**, **smoke checks**, **Playwright-прогоны** и **migration verification** обязаны использовать изолированные или временные базы данных. **Никогда** не указывай автотестам путь к user ARK DB.
:::

Это политика репо, закреплена в корневом `AGENTS.md` и продублирована в `AGENTS.md` каждого приложения.

## Почему

Автоматические прогоны против реальной ARK DB пользователя рискуют:

- Испортить данные (миграции, INSERT с конфликтом, ошибочный DELETE).
- Полностью удалить данные (тест очищает БД перед прогоном).
- Засорить sync: тестовые версии записей улетят на пиры.

Это **необратимо**. Один прогон Playwright против `%APPDATA%\Kosmos\ark.db` может убить накопленные за месяцы записи.

## Где хранить тестовые БД

| Путь                                     | Когда                                           |
| ---------------------------------------- | ----------------------------------------------- |
| `.agent/tasks/<TASK_ID>/smoke/`          | task-локальные артефакты proof-loop             |
| `apps/<app>/.tmp/`                       | локальные unit/integration прогоны разработчика |
| `apps/<app>/.e2e/`                       | Playwright e2e                                  |
| OS temp (`os.tmpdir()`, `$env:TEMP`)     | эфемерные одноразовые smoke                     |
| `dist/`, `build/`, `coverage/`           | ❌ нет — попадает в коммит и в артефакты сборки |
| user data dir (`%APPDATA%`, `~/.config`) | ❌ нет — это user DB                            |

`.tmp` и `.e2e` должны быть в `.gitignore` каждого приложения.

## Как передать путь в тест

Все приложения и smoke-скрипты **обязаны** принимать явный путь к БД через флаг или env. Не дефолтиться в `%APPDATA%`.

### Через env

```powershell
$env:KOSMOS_SMOKE_ROOT = ".agent\tasks\2026-04-26-ark-app-completion\smoke"
New-Item -ItemType Directory -Force -Path $env:KOSMOS_SMOKE_ROOT | Out-Null
$env:ARK_DB_PATH = "$env:KOSMOS_SMOKE_ROOT\usage-tracker\ark.db"
```

### `KOSMOS_DATA_DIR` — backend-wide override (Playwright/e2e)

`kepler-backend` поддерживает env-переменную `KOSMOS_DATA_DIR`, которая
**полностью** заменяет базовую директорию backend'а (`%APPDATA%\Kosmos\` /
`$XDG_CONFIG_HOME/Kosmos/`). Под этим dir живут:

- `kepler.lock.json` — lock-файл discovery.
- `kepler-singleton.lock.db` — singleton guard.
- `ark.db` — дефолтная ARK DB (если не задан `KOSMOS_DB_PATH`).
- `device.id`, `space.id` и прочая state-метадата.

Это **единственный** безопасный способ изолировать Playwright/e2e от user data:

```powershell
$env:KOSMOS_DATA_DIR = "D:\repo\tests\.e2e\my-spec"
$env:KOSMOS_TEST_MODE = "1"
playwright test
```

Реализовано в `services/kepler-backend/src/lock_file.rs`
(`kosmos_data_dir()`). Test helper `tests/e2e/helpers/launch.ts` форсит
эту переменную для всех specs и явно отказывается принимать путь внутри
`%APPDATA%`.

## Dev mode изоляция

`bun run --cwd shell dev` **не пишет** в `%APPDATA%\Kosmos\` — он
использует `%APPDATA%\Kosmos-dev\` (отдельная директория). Это
гарантирует, что разработка / эксперименты с кодом не повреждают данные,
которые видит production install.

Resolution chain в `shell/electron/data-dir.ts` `keplerDataDir()`:

1. `KOSMOS_DATA_DIR` env (если set) — absolute path. Используется для
   Playwright e2e (test isolation).
2. `VITE_DEV_SERVER_URL` set (`bun run --cwd shell dev` через Vite) →
   `<appData>\Kosmos-dev`.
3. Иначе (production install): `<appData>\Kosmos`.

Shell спавнит kepler-backend с `KOSMOS_DATA_DIR=<resolved>` env, чтобы
shell и backend смотрели на один dir. Все hardcoded `path.join(appData,
"Kosmos")` в shell заменены вызовом `keplerDataDir()`.

**Миграция существующих dev данных** (опционально):

```powershell
Copy-Item -Recurse "$env:APPDATA\Kosmos" "$env:APPDATA\Kosmos-dev"
```

После этого dev mode подхватит ту же DB. Production install продолжит
видеть оригинал.

### Через CLI-флаг

```powershell
node --experimental-strip-types apps\dashboard\scripts\seedSmokeDb.ts `
  --db-path "$env:KOSMOS_SMOKE_ROOT\dashboard\smoke-dashboard.db"
```

### В Playwright globalSetup

Создавай smoke-БД в `globalSetup` под `apps/<app>/.e2e/`. Передавай путь через env-переменную, которую читает Electron main при `app.isPackaged ? userData : env.ARK_DB_PATH`.

## packaged smoke

После Phase B-D большинство приложений живут как Vue-extensions в Kepler shell. Для packaged Electron-приложений (Kepler shell, Eden), которые умеют делать packaged smoke, паттерн тот же: собирается unpacked Windows-бандл и запускается с **временными** `APPDATA`, `LOCALAPPDATA`, `ARK_DB_PATH` под `<app>/.e2e/packaged-smoke/`.

Это нужно потому, что packaged Electron читает реальные `%APPDATA%` пути — приходится подменять весь user dir, а не только `ARK_DB_PATH`.

## Что точно нельзя

- ❌ Запускать миграцию/backfill против реальной ARK DB ради «проверить, как сработает».
- ❌ Запускать Playwright без `KOSMOS_SMOKE_ROOT` или эквивалентного override.
- ❌ Делать тестовый Vitest, который дефолтится в `app.getPath('userData')`.
- ❌ Использовать `:memory:` как «достаточную изоляцию» — некоторые миграции не воспроизводятся в in-memory; используй файловую БД во временной папке.

## Проверка в code review

Если в новом тесте/скрипте видишь:

- Дефолт пути в user data dir — **reject**, пусть передаст явный путь.
- Захардкоженный путь типа `C:\\Users\\me\\AppData\\...` — **reject**.
- Отсутствие cleanup `.tmp` / `.e2e` в `.gitignore` — **reject**.

## Связанные документы

- [Smoke-матрица ARK](/reference/smoke-matrix) — конкретные команды smoke.
- [Proof loop](/concepts/proof-loop) — task-локальные `smoke/` папки.
- Корневой `AGENTS.md` — формальная политика.
