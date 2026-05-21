# MIGRATION 2026-05-14 — Brand swap Kepler ↔ Kosmos

**Дата**: 2026-05-14
**Статус**: Required user action
**Скоуп**: пользовательские данные, реестр, установочная директория

## Контекст

14 мая 2026 в репозитории выполнен global rebrand:

- **Kepler** теперь — имя launcher'а (раньше так назывался ecosystem / монорепо).
- **Kosmos** теперь — имя ecosystem / монорепо (раньше так назывался launcher).

Старая раскладка:

```
Kepler   = ecosystem (apps + ARK runtime + packages)
Kosmos   = launcher (отдельное приложение, host для ark-core-rpc)
```

Новая раскладка:

```
Kosmos   = ecosystem (apps + ARK runtime + packages)
Kepler   = launcher (отдельное приложение, host для ark-core-rpc)
```

## Что затронуто

| Категория          | До                                  | После                               |
| ------------------ | ----------------------------------- | ----------------------------------- |
| Ecosystem AppData  | `%APPDATA%\Kepler\`                 | `%APPDATA%\Kosmos\`                 |
| ARK DB             | `%APPDATA%\Kepler\ark.db`           | `%APPDATA%\Kosmos\ark.db`           |
| Installer dir      | `%LOCALAPPDATA%\Kepler\Kosmos\`     | `%LOCALAPPDATA%\Kosmos\Kepler\`     |
| Launcher lock-file | `kosmos.lock.json`                  | `kepler.lock.json`                  |
| Singleton lock     | `kosmos-singleton.lock.db`          | `kepler-singleton.lock.db`          |
| Device id          | `kosmos-device-id.txt`              | `kepler-device-id.txt`              |
| HKCU Run key       | `KeplerKosmos`                      | `KosmosKepler`                      |
| TS package         | `@kepler/ark`, `@kepler/visuals`    | `@kosmos/ark`, `@kosmos/visuals`    |
| Env vars           | `KEPLER_DB_PATH`, `KEPLER_SPACE_ID` | `KOSMOS_DB_PATH`, `KOSMOS_SPACE_ID` |
| Backend service    | `services/kosmos-backend`           | `services/kepler-backend`           |

Полный список изменений в коде делается параллельно агентами 1–4 (см. swap PR). Этот документ описывает только **пользовательские данные**.

## User action required

> [!IMPORTANT]
> Выполнить ровно один раз, после установки swap'нутой версии репозитория, **до** первого запуска приложений из новой раскладки.

1. **Закройте все приложения экосистемы.**
   - Eden, Delphi, Arrancador, Dashboard, Horologion.
   - Запущенный Kosmos launcher (старый бренд).
   - `ark-core-rpc`, `usage-tracker` (фон).

2. **Сделайте резервную копию** `%APPDATA%\Kepler\`. Скопируйте куда угодно — внешний диск, другая папка, неважно. Это страховка на случай, если миграция пойдёт не так.

3. **Запустите миграционный скрипт**:

   ```powershell
   pwsh -File scripts\migrate-kepler-to-kosmos.ps1
   ```

   Сначала можно прогнать в dry-run режиме:

   ```powershell
   pwsh -File scripts\migrate-kepler-to-kosmos.ps1 -WhatIf
   ```

4. **Установите новый Kepler launcher** (после swap'а это бывший Kosmos launcher, переименованный). Если launcher уже стоял в `%LOCALAPPDATA%\Kepler\Kosmos\`, миграционный скрипт перенесёт его в `%LOCALAPPDATA%\Kosmos\Kepler\`. HKCU autorun запись будет пересоздана только если новый `kepler.exe` найден в target.

5. **Запустите Kepler launcher**. Он должен поднять `ark-core-rpc` против `%APPDATA%\Kosmos\ark.db`. Откройте любое приложение (Eden / Delphi / Horologion) и проверьте, что данные на месте.

## Что делает скрипт

Атомарные шаги (`scripts/migrate-kepler-to-kosmos.ps1`):

1. Detect: если `%APPDATA%\Kepler\` отсутствует — exit 0.
2. Если `%APPDATA%\Kosmos\` уже существует и непуст — abort, требует `-Force`.
3. Stop процессов: `kosmos`, `kosmos-backend`, `kosmos-watcher`, `ark-core-rpc`, `usage-tracker`.
4. `Move-Item %APPDATA%\Kepler -> %APPDATA%\Kosmos` (атомарно; cross-volume fallback на copy+remove).
5. Rename внутри target: `kosmos.lock.json` → `kepler.lock.json`, и две похожие пары.
6. HKCU Run: удалить `KeplerKosmos`, добавить `KosmosKepler` (только если новый launcher уже установлен).
7. Move installer dir: `%LOCALAPPDATA%\Kepler\Kosmos\` → `%LOCALAPPDATA%\Kosmos\Kepler\`.
8. Print summary.

Идемпотентность: повторный запуск после успешной миграции ничего не делает (source отсутствует → exit 0).

Smoke test: `scripts/migrate-kepler-to-kosmos-smoke.ps1` гоняет миграцию в temp-песочнице с фейковыми данными и проверяет результат. Реальные данные пользователя не трогает.

## Risk и rollback

**Риск**: основная операция — `Move-Item` для директории целиком, что атомарно в пределах одного тома. Cross-volume переезд (если `%APPDATA%` на другом физическом диске, чем target) использует copy + remove fallback — там есть короткое окно, где данные дублированы.

**Rollback** (пока новые приложения не запущены и не успели записать в `%APPDATA%\Kosmos\ark.db`):

```powershell
# Закрыть всё.
Move-Item $env:APPDATA\Kosmos $env:APPDATA\Kepler

# Вернуть имена файлов:
Rename-Item "$env:APPDATA\Kepler\kepler.lock.json"          kosmos.lock.json
Rename-Item "$env:APPDATA\Kepler\kepler-singleton.lock.db"  kosmos-singleton.lock.db
Rename-Item "$env:APPDATA\Kepler\kepler-device-id.txt"      kosmos-device-id.txt

# Installer:
Move-Item "$env:LOCALAPPDATA\Kosmos\Kepler" "$env:LOCALAPPDATA\Kepler\Kosmos"

# HKCU:
Remove-ItemProperty HKCU:\Software\Microsoft\Windows\CurrentVersion\Run KosmosKepler -ErrorAction SilentlyContinue
```

После rollback можно вернуться на pre-swap версию кода (git checkout до swap-коммитов).

**Что НЕЛЬЗЯ откатить простым переименованием обратно**:

- Если новые приложения уже запустились и записали в `%APPDATA%\Kosmos\ark.db` — данные в новом расположении свежее. Откат сделает потерю этих изменений.

## Связанные документы

- `scripts/migrate-kepler-to-kosmos.ps1` — сам скрипт.
- `scripts/migrate-kepler-to-kosmos-smoke.ps1` — smoke-тест.
- `scripts/check-swap-completeness.ps1` — verify, что все упоминания старых токенов в коде заменены.
