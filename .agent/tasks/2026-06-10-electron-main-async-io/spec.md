# 2026-06-10 — Electron main: убрать синхронный I/O с горячих путей

## Контекст

Аудит блокировок (3 агента: архитектура / Electron / Rust, 2026-06-10) показал:

- **Rust-рантайм уже починен** задачей `2026-06-09-app-index-icon-throttle`: lazy icon
  extraction, throttled queue, background-priority `spawn_blocking`, никакого inline base64
  в WS-ответах. Оставшиеся Rust-находки — LOW (batch progress-mutex в `file_index`),
  в scope этой задачи не входят.
- **Оставшиеся блокировки — в Electron main process.** Синхронный fs/child_process I/O
  выполняется прямо в IPC/protocol handler'ах: пока он идёт, main process не обрабатывает
  события → зависают ВСЕ окна shell'а (launcher, Dashboard, extension windows).

Верифицированные находки (по убыванию серьёзности):

| #   | Где                                                                                                                                                                                | Что блокирует                                                                                                            | Контекст вызова                                     |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------- |
| 1   | `platform/desktop/electron/extension-installer.ts:337` (`copyDirSync`), вызовы на :331, :414, :462, :504                                                                           | Рекурсивное синхронное копирование директории (backup / install / rollback расширения; может включать большие payload'ы) | IPC install/update/rollback                         |
| 2   | `extension-installer.ts:76` (`readFileSync(zipPath)`), `:138` (`writeFileSync` per-entry распаковки), `:354-371` (`pruneBackups`: `readdirSync`+`statSync`+`rmSync`)               | ZIP целиком в память синхронно; распаковка по-файлово синхронно                                                          | IPC install                                         |
| 3   | `diagnostics.ts:74` (`copyFileSync` в цикле `copyRecentFiles`), `:254-255` (`writeFileSync`), `:270` (`spawnSync powershell Compress-Archive`), `:327` (`copyFileSync` результата) | Сборка diagnostics bundle: копирование логов/дампов + ZIP синхронно — секунды на больших логах                           | IPC `kepler:diagnostics:*`                          |
| 4   | `settings-window.ts:258` (`execFileSync("reg.exe", ...)`)                                                                                                                          | Запись в реестр (toggle autostart), 100–500 мс                                                                           | IPC из Settings                                     |
| 5   | `main.ts:824` (`readFileSync(iconPath)` в `kosmos-icon://` protocol handler)                                                                                                       | Чтение PNG на cache-miss; handler уже async (await ark) — sync read лишний                                               | protocol handler, частый при первом рендере списков |
| 6   | `extension-marketplace.ts:114` (`readFileSync` для hash)                                                                                                                           | Хеширование скачанного архива целиком из памяти                                                                          | IPC install path                                    |

Не трогаем (осознанно): мелкие JSON state read/write (`focus-widget.ts`, `dashboard-window.ts`,
`settings-window.ts` settings, `clipboard-history-store.ts` startup load) — файлы < десятков КБ,
однократные; `extensionIconDataUri` / `readManifestIconAsDataUri` в `extension-host.ts` —
маленькие файлы + mtime-кеш; `instance.ts` migration — однократный startup path.

## Scope

В scope:

- Перевести пункты 1–6 на async API (`fs.promises.*`, `fs.promises.cp`, async `spawn`/`execFile`),
  сделав содержащие функции `async` и поправив call sites (все они — IPC/protocol handler'ы,
  уже async-совместимые).
- Сохранить семантику 1:1: тот же layout файлов, тот же порядок backup → install → rollback,
  та же политика pruning, те же коды ошибок/fallback'и.
- Regression coverage: unit-тесты на изменённую логику там, где она выделяется в чистые
  функции (vitest, рядом с существующими `*.test.ts` в `platform/desktop/electron/`).

Не в scope:

- Любой попутный рефакторинг (структура модулей, переименования).
- Новые npm-зависимости (ZIP остаётся через PowerShell, но async `spawn`).
- Rust-side оптимизации (LOW из аудита) — отдельной задачей при необходимости.
- Renderer-side изменения.

## Acceptance Criteria

**AC1.** В `extension-installer.ts` нет `copyDirSync` / `readFileSync` / `writeFileSync` /
`rmSync` / `readdirSync` / `statSync` на путях install / backup / rollback / prune:
копирование через `fs.promises.cp({ recursive: true })`, остальное через `fs.promises.*`.
Порядок операций (backup → validate → swap → prune; rollback из backup при ошибке) не изменён.

**AC2.** Сборка diagnostics bundle (`diagnostics.ts`) не содержит синхронных fs-вызовов и
`spawnSync`: копирование файлов — `fs.promises`, ZIP — async `spawn("powershell.exe", ...)`
с ожиданием exit, сохранение результата — `fs.promises.copyFile`.

**AC3.** `kosmos-icon://` protocol handler в `main.ts` читает байты иконки через
`await fs.promises.readFile`; кеш `appIconBytesCache` и заголовки ответа не изменены.

**AC4.** Toggle autostart в `settings-window.ts` использует async `execFile` (promisified);
ошибка reg.exe по-прежнему репортится так же, как сейчас.

**AC5.** `extension-marketplace.ts` хеширует файл без `readFileSync` (async read или stream).

**AC6.** `bun test electron/` (из `platform/desktop/`, bun:test) зелёный — baseline:
14 pass / 0 fail (`baseline.md`); `bun run ark:guard:writes` зелёный; `bun run lint` и
`bun run desktop:typecheck` без новых ошибок (baseline: чисто). Существующие тесты не
отключены и не ослаблены.

**AC7.** grep по изменённым файлам не находит вновь добавленных `*Sync(`-вызовов
(кроме осознанно оставленных вне scope, перечисленных выше).

## План имплементации (для субагентов)

Каждый этап — отдельный коммит, один логический change. Исполнитель: **haiku**, если не
указано иное. Перед правками каждому агенту читать только нужный файл + этот spec.

1. **Этап A (haiku):** `extension-installer.ts` — AC1 + AC5-часть про `:76/:138`.
   Самый большой объём, но механический. Внимание: функции становятся async по цепочке —
   проверить все call sites через grep.
2. **Этап B (haiku):** `diagnostics.ts` — AC2. `spawnSync` → `spawn` + promise на exit,
   таймаут сохранить, stderr — в лог как сейчас.
3. **Этап C (haiku):** `main.ts` protocol handler (AC3) + `settings-window.ts` (AC4) +
   `extension-marketplace.ts` hash (AC5). Три точечные правки.
4. **Этап D (sonnet):** verifier — прогнать AC6/AC7, перечитать diff целиком на предмет
   потерянных `await`, изменённого порядка операций, проглоченных ошибок. Evidence в
   `evidence.md` этого таска.

Риски:

- Потерянный `await` при переводе на async — молчаливая гонка (install продолжится до
  завершения backup). Verifier обязан проверить каждый перевод call site.
- `fs.promises.cp` на Windows + junction'ы в `node_modules/` — backup расширения копирует
  и их; поведение `copyDirSync` сейчас следует symlink'ам через `readdirSync` без
  `withFileTypes`-symlink-обработки? Проверить на этапе A фактическое поведение и сохранить
  его (`dereference` опция `cp`).
- E2e headless-контракт не затронут (окна не создаём), но запуск vitest обязателен.

## Заметка по симптому

Блокировки main process вешают **окна Kosmos**, а не весь компьютер. Если наблюдаются
зависания всей системы — это, скорее всего, был уже исправленный icon-storm в Rust
(disk/CPU saturation) либо иная причина; после этой задачи симптом надо перепроверить
и при сохранении — копать отдельно (WPR trace из Phase 3 diagnostics).
