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

| Путь | Когда |
|---|---|
| `.agent/tasks/<TASK_ID>/smoke/` | task-локальные артефакты proof-loop |
| `apps/<app>/.tmp/` | локальные unit/integration прогоны разработчика |
| `apps/<app>/.e2e/` | Playwright e2e |
| OS temp (`os.tmpdir()`, `$env:TEMP`) | эфемерные одноразовые smoke |
| `dist/`, `build/`, `coverage/` | ❌ нет — попадает в коммит и в артефакты сборки |
| user data dir (`%APPDATA%`, `~/.config`) | ❌ нет — это user DB |

`.tmp` и `.e2e` должны быть в `.gitignore` каждого приложения.

## Как передать путь в тест

Все приложения и smoke-скрипты **обязаны** принимать явный путь к БД через флаг или env. Не дефолтиться в `%APPDATA%`.

### Через env

```powershell
$env:KOSMOS_SMOKE_ROOT = ".agent\tasks\2026-04-26-ark-app-completion\smoke"
New-Item -ItemType Directory -Force -Path $env:KOSMOS_SMOKE_ROOT | Out-Null
$env:ARK_DB_PATH = "$env:KOSMOS_SMOKE_ROOT\usage-tracker\ark.db"
```

### Через CLI-флаг

```powershell
node --experimental-strip-types apps\dashboard\scripts\seedSmokeDb.ts `
  --db-path "$env:KOSMOS_SMOKE_ROOT\dashboard\smoke-dashboard.db"
```

### В Playwright globalSetup

Создавай smoke-БД в `globalSetup` под `apps/<app>/.e2e/`. Передавай путь через env-переменную, которую читает Electron main при `app.isPackaged ? userData : env.ARK_DB_PATH`.

## packaged smoke (Arrancador пример)

Arrancador собирает unpacked Windows-бандл и запускает `release/win-unpacked/arrancador.exe` с **временными** `APPDATA`, `LOCALAPPDATA`, `ARK_DB_PATH` под `apps/arrancador/.e2e/packaged-smoke`:

```powershell
bun run --cwd apps/arrancador smoke:packaged
```

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
