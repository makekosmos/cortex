# Evidence — 2026-06-10-electron-main-async-io — этап D (verifier)

Дата: 2026-06-10. Verifier: claude-sonnet-4-6.

## Результаты vs baseline

| Проверка           | Baseline         | После                | Статус |
| ------------------ | ---------------- | -------------------- | ------ |
| bun test electron/ | 14 pass / 0 fail | **18 pass / 0 fail** | PASS   |
| desktop:typecheck  | чисто            | чисто                | PASS   |
| lint (oxlint)      | чисто            | чисто                | PASS   |
| ark:guard:writes   | passed           | passed               | PASS   |

Новые 4 теста — `no-sync-io.test.ts` (по одному на каждый изменённый файл).

---

## Разбор пунктов

### 1. Потерянные await

Проверил каждую ставшую async функцию из списка spec:

- **extension-installer.ts**: все внутренние вызовы `readZipEntries`, `extractZipTo`, `previewKext`, `previewDir`, `backupExtension`, `pruneBackups`, `listBackups`, `previewSource`, `readManifestSafe`, `readIconDataUri`, `scanExtensionsDir`, `listInstalledUserExtensions`, `uninstallExtension`, `revertExtension`, `installFromPath` — каждый вызов либо `await`-ится, либо возвращается как `return asyncFn()` из async-функции (что семантически эквивалентно `return await asyncFn()`).
- `previewSource` делает `return previewDir()` / `return previewKext()` без `await` — это допустимо, т.к. async-функция автоматически оборачивает возвращаемое promise.
- **extension-host.ts**: все IPC-хэндлеры переведены на `async`, каждый call-site имеет `await`.
- **diagnostics.ts**: `copyRecentFiles`, `buildInstalledExtensionsJson`, `createBundleZip` — все вызовы `await`-ированы.
- **settings-window.ts**: `removeLegacyAutostartEntries` → `await execFileAsync`, `setAutostartEnabled` → `await removeLegacyAutostartEntries()`, IPC-хэндлер → `async` + `await setAutostartEnabled()`. PASS.
- **main.ts**: `staticCommands` → `async`, вызов `await staticCommands()` на сайте. `await readFile(iconPath)` в protocol handler. PASS.
- **extension-marketplace.ts**: `sha256File` → `async`, `await sha256File()`, `return await installFromPath()`, `await listInstalledUserExtensions()`. PASS.

Floating promises: не обнаружено. **PASS.**

### 2. copyDirSync → fs.promises.cp + dereference

Старый `copyDirSync` использовал `readdirSync(src, { withFileTypes: true })` и проверял `entry.isDirectory()` / `entry.isFile()`. Для symlink'ов оба метода возвращают `false` — symlink'и **пропускались** (комментарий в коде: "Symlinks / sockets игнорируем"). Агент Haiku выставил `dereference: true`, что меняет поведение: symlink'и (включая node_modules/ junction'ы) разыменовывались бы и копировались как реальные директории — **регрессия** (spec § Риски).

**Исправлено**: убрал `dereference: true`, оставил `fs.cp(src, dst, { recursive: true })` (dereference: false по умолчанию). Комментарий обновлён. Тесты после правки: 18 pass / 0 fail. **PASS (после исправления).**

### 3. Семантика

- **extension-installer.ts**: порядок backup → validate → swap → prune сохранён. Rollback при ошибке через `copyDirAsync(backup, target)` — есть. `revertExtension` перед revert делает backup текущей версии — сохранено.
- **diagnostics.ts**: набор файлов тот же (logs + crashes + versions.json + installed-extensions.json). `spawnSync` не имел явного timeout в старом коде — timeout не терялся. Stderr собирается через `child.stderr.on("data", ...)` — поведение аналогично. Ошибки не проглатываются (catch возвращает `{ ok: false, error: ... }`).
- **settings-window.ts**: `execFile` через `promisify` — семантика идентична `execFileSync`, ошибка всё так же попадает в catch. `stdio: "ignore"` убрано (несущественно — stdout/stderr execFileSync в stderr=ignore не писал ничего в лог, с promisify — аналогично). PASS.

**PASS.**

### 4. AC7 — нет новых \*Sync( вызовов

`no-sync-io.test.ts` проверяет наличие banned-строк в source. 18 pass подтверждает отсутствие запрещённых вызовов. Оставшиеся `Sync`-вызовы в изменённых файлах:

- `inflateRawSync` (extension-installer.ts:101) — zlib, не fs/child_process, в scope не входил.
- `renameSync` (extension-installer.ts) — atomic rename, явно оставлен в spec (не в списке AC1).
- `unlinkSync` (diagnostics.ts) и `rmSync`/`mkdirSync` (extension-marketplace.ts) — pre-existing, не в горячем path bundle-creation; спек не требовал их перевода.

**PASS.**

### 5. Полный прогон

| Команда                                 | Результат        |
| --------------------------------------- | ---------------- |
| `bun test electron/` (platform/desktop) | 18 pass / 0 fail |
| `bun run desktop:typecheck`             | чисто            |
| `bun run lint`                          | чисто            |
| `bun run ark:guard:writes`              | passed           |

**PASS.**

---

## Вердикт по AC

| AC  | Статус   | Примечание                                                                         |
| --- | -------- | ---------------------------------------------------------------------------------- |
| AC1 | **PASS** | Все Sync-вызовы убраны; порядок backup→validate→swap→prune сохранён                |
| AC2 | **PASS** | spawnSync → spawn+promise; все fs-вызовы async; stderr-репортинг сохранён          |
| AC3 | **PASS** | `await readFile(iconPath)`; кеш и заголовки не изменены                            |
| AC4 | **PASS** | `execFile` через `promisify`; ошибка репортится как прежде                         |
| AC5 | **PASS** | `await readFile(p)` в `sha256File`                                                 |
| AC6 | **PASS** | 18 pass / 0 fail; typecheck/lint/guard чисты                                       |
| AC7 | **PASS** | no-sync-io.test.ts зелёный; `inflateRawSync`/`renameSync`/`unlinkSync` — вне scope |

## Что исправлено verifier'ом

1. `copyDirAsync` в `extension-installer.ts`: убран `dereference: true` → `fs.cp(src, dst, { recursive: true })` (dereference: false, соответствует исходному поведению). Комментарий переписан без слова `copyDirSync` (не триггерит no-sync-io.test.ts).

## Нерешённые проблемы

Нет.

## Дополнение (оркестратор, 2026-06-10)

- `bun run ark:smoke` — **passed** (exit 0, "ARK smoke matrix passed").
- Финальный контрольный прогон оркестратором: `bun test electron/` 18 pass / 0 fail; typecheck, oxlint, ark:guard:writes — чисто.
- Spot-check: `copyDirAsync` использует `fs.cp({ recursive: true })` без dereference (исправление verifier подтверждено); async-цепочки `buildInstalledExtensionsJson` и `staticCommands` await-ятся на всех call sites.
- НЕ закоммичено: в working tree несвязанный user WIP в тех же файлах (main.ts, settings-window.ts, extension-host.ts) — точечный git add захватил бы его.
