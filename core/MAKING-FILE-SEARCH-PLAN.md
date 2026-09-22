# План: Kepler File Search — Phase 1 + Phase 2

Дата: 2026-05-23.

Источник правды для всей работы по file search. Любые изменения архитектуры или scope — сначала сюда, потом в код.

## 1. Цель и контекст

**Симптом пользователя.** Поиск файлов в Kepler находит только файлы под `%APPDATA%\Roaming\…`, не находит проекты на `C:\` или `D:\`. Подтверждено эмпирически.

**Текущее состояние кода.**

- `services/kepler-backend/src/file_index/scanner.rs::default_roots()` возвращает `["C:\\", "D:\\"]` — оба диска целиком.
- WalkDir идёт alphabetically depth-first → попадает в `C:\Users\<u>\AppData\` (потому что `Users` алфавитно раньше `Windows`) → застревает в гигабайтах кэша Chrome/Edge/VSCode → **никогда** не доходит до реальных проектов.
- Fast-path через `kepler-focus-svc` (NTFS USN) требует admin — служба не установлена в дефолтной конфигурации.
- Fallback на `Volume::new(\\.\C:)` тоже требует admin — фейлится.
- Итог: silent slow user-mode fallback который не выходит за пределы `AppData` за обозримое время.

**Цель.** Сделать file search **работающим из коробки без admin**, по тому же UX-паттерну что Raycast — `%USERPROFILE%` целиком как default scope, с smart ignore patterns. Опционально позже — opt-in «Быстрый поиск» с MFT/USN через службу.

## 2. Эмпирика Raycast (фактическая картина)

Verified из скриншотов Settings (2026-05-23):

- **Default scope** — весь `%USERPROFILE%` **одним root'ом**, не subset `Documents/Downloads/Desktop`.
- **Шум душится через ignore patterns**, не через ограничение roots:
  - `*.tmp`, `*.temp`
  - `node_modules`
  - `**/tmp/**`, `**/temp/**`
  - `**/[Cc]ache/**`, `**/[Cc]aches/**`
  - **`**/AppData/**`** — критичный, без него `%USERPROFILE%` = 100k+ файлов мусора
- `.gitignore` / `.ignore` / `.rayignore` respect — **ON by default**.
- Include Hidden Files — **OFF by default**, отдельный toggle.
- UI: один плоский список scopes + `+` (folder picker) + `−` (remove). Никаких per-root settings (depth, follow-symlinks).
- **Fast File Scanner toggle (changelog v0.29) в текущей сборке Settings ОТСУТСТВУЕТ.** Либо удалён, либо contextual (промпт при добавлении большого root). Эмпирика поведения при добавлении `D:\` целиком — **pending** (см. § 10).
- Все процессы Raycast user-mode. Служб в `services.msc` нет. `Raycast.UIAccess.exe` — про Windows UIAccess для global hotkeys, **не filesystem**.

**Вывод.** Raycast делает Phase 1 (user-mode + smart defaults) в дефолтной поставке. MFT — была/есть как opt-in расширение, не дефолт. Наш план должен зеркалить это разделение.

## 3. Phase 1: scope

**В скоупе.**

- Refactor `scanner.rs` под `trait FileScanner` (strategy pattern) с одной реализацией `UserModeScanner`.
- Заменить дефолтные roots с `[C:\, D:\]` на `[%USERPROFILE%]`.
- Реализовать ignore patterns (hardcoded fast-path + `ignore` crate для `.gitignore`/`.ignore`).
- Добавить toggle «Учитывать `.gitignore`» (default ON), «Показывать скрытые файлы» (default OFF) и «Ускоренный NTFS-режим» (default OFF) в settings store + UI.
- Settings UI «Папки поиска» в Kepler shell: плоский список scopes + add/remove.
- Settings UI «Ignore patterns»: пользовательские patterns типа `*.tmp`, `*.log`, `**/Cache/**` с add/remove.
- Persistence: settings храним в SQLite `file_index_settings` table (уже существует, см. `file_index/store.rs:25-28`).
- Регрешн-тесты на новые поведения.
- Postmortem entry в `docs-site/agents/postmortems.md`, ссылающийся на оригинальный bug (Kepler file search показывает только `%APPDATA%` — UNRESOLVED → RESOLVED).

**Вне скоупа (явно).**

- Никакого нового MFT/USN scanner'а — existing `scanner/ntfs.rs` остаётся только как opt-in accelerated mode.
- Никаких stubs или TODO-комментариев которые «заведут» в Phase 2 territory.
- Никакого disabled-toggle «Быстрый поиск (Phase 2)» в UI; реальный toggle называется «Ускоренный NTFS-режим» и управляет existing fast path.
- Никакого нового pipe-protocol operation в focus-svc.
- Никакого `kepler-index-svc` (отдельной службы). Это Phase 2 если он вообще нужен.
- Никаких изменений в installer — Phase 1 работает на текущей user-mode установке.
- Никакого full-text content search — только filename + path matching (как сейчас).
- Никаких network drive / mounted volume specials.

## 4. Phase 1: архитектурные решения

### 4.1 `trait FileScanner` (strategy pattern)

```rust
// services/kepler-backend/src/file_index/scanner/mod.rs
pub trait FileScanner: Send + Sync {
    /// Однократный enumeration roots → отдаём список IndexedFile.
    fn scan(&self, roots: &[PathBuf], opts: &ScanOptions) -> Result<Vec<IndexedFile>>;
    /// Имя для диагностики/логов ("user-mode-walk").
    fn name(&self) -> &'static str;
}

pub struct ScanOptions {
    pub respect_gitignore: bool,
    pub include_hidden: bool,
    pub extra_ignore_patterns: Vec<String>, // user-added в Settings
    pub ntfs_accelerated: bool,
}

pub struct UserModeScanner {
    // owns nothing privileged
}

impl FileScanner for UserModeScanner { … }
```

`scan_roots()` в `scanner.rs` сейчас — свободная функция. Превратить её в impl `UserModeScanner::scan`. Сохранить backward-compat сигнатуру для существующих вызовов (`file_index/mod.rs::rescan`).

**Скоуп guard.** В этом коммите — **только** `UserModeScanner`. Никаких других impl'ов trait'а. Если кажется что «логично сразу написать `MftScanner` хотя бы как unimplemented!()» — стоп, не делаем. Trait сам по себе зарезервирован для Phase 2, его одного достаточно.

### 4.2 `ignore` crate для gitignore respect

Добавить в `services/kepler-backend/Cargo.toml`:

```toml
ignore = "0.4"
```

Это [BurntSushi/ripgrep/ignore](https://github.com/BurntSushi/ripgrep/tree/master/crates/ignore), mature и проверенный. Использовать `WalkBuilder` вместо текущего walkdir где это уместно. `WalkBuilder` сам обрабатывает `.gitignore` / `.ignore` / `.git/info/exclude` / global gitignore и быстрый custom override list.

```rust
use ignore::WalkBuilder;

let mut wb = WalkBuilder::new(root);
wb.git_ignore(opts.respect_gitignore)
  .git_global(opts.respect_gitignore)
  .git_exclude(opts.respect_gitignore)
  .hidden(!opts.include_hidden)
  .add_custom_ignore_filename(".rayignore"); // для совместимости с миграциями из Raycast
for pat in HARDCODED_IGNORES {
    wb.add_ignore(pat)?;
}
```

Hardcoded fast-path нужен потому что `ignore` crate начинает применять `.gitignore` правила только после того как доходит до конкретного gitignore-файла. Hardcoded blacklist режет `node_modules`/`target`/`AppData` сразу при traversal, до прочтения любых gitignore.

### 4.3 Hardcoded ignore patterns (final list)

Verbatim в коде:

```rust
const HARDCODED_IGNORES: &[&str] = &[
    // Raycast default — копируем как есть
    "*.tmp",
    "*.temp",
    "node_modules",
    "**/tmp/**",
    "**/temp/**",
    "**/[Cc]ache/**",
    "**/[Cc]aches/**",
    "**/AppData/**",
    // Прагматичные дополнения (быстрее чем читать gitignore'ы):
    "**/target/**",     // Rust
    "**/dist/**",       // generic build
    "**/build/**",      // generic build
    "**/.next/**",      // Next.js
    "**/.nuxt/**",      // Nuxt
    "**/.git/**",       // git internals
    "**/__pycache__/**",// Python
];
```

Hardcoded list остаётся default baseline. Пользовательские patterns через Settings добавляются поверх этого списка и persist'ятся в `file-index.db`; `.rayignore` / `.ignore` остаются дополнительным способом локальной настройки на уровне каталога.

### 4.4 Default roots

```rust
fn default_roots() -> Vec<PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir());
    home.into_iter().collect()
}
```

**Только `%USERPROFILE%`.** Один root. Эмпирика Raycast = source of truth. Без `D:\Personal` и т.п. в дефолтах — user сам добавит в Settings если нужно (мы тоже добавим себе после ship'а).

`KEPLER_FILE_INDEX_ROOTS` env var override уже существует (`scanner.rs`, для test mode). Сохранить как есть.

### 4.5 Hidden files и gitignore toggles

Persist в `file_index_settings` таблице (схема уже есть, `store.rs:25-28`). Добавить две новые keys:

- `respect_gitignore` (default `"1"`)
- `include_hidden` (default `"0"`)

Method'ы `FileStore::get_respect_gitignore`/`set_*` и `get_include_hidden`/`set_*` — по аналогии с существующим `exclude_noisy_folders`. Возможно `exclude_noisy_folders` deprecated после Phase 1 (его роль теперь играет hardcoded blacklist), но не удалять — pre-existing data backward-compat.

### 4.6 Roots persistence

Список scopes хранить в новой таблице `file_index_roots`:

```sql
CREATE TABLE IF NOT EXISTS file_index_roots (
    path TEXT PRIMARY KEY,
    added_at INTEGER NOT NULL
);
```

При первой инициализации (таблица пустая после migration) — заполнить дефолтным `%USERPROFILE%`. После — пользователь управляет через Settings UI.

### 4.7 NTFS accelerated mode

`ntfs_accelerated` хранится в `file_index_settings`, default `"0"`.

- OFF — scanner не заходит в `scanner/ntfs.rs`, всегда user-mode traversal.
- ON — scanner пробует existing NTFS fast path только для drive roots (`C:\`, `D:\`). Если service/direct volume недоступны, логирует warning и fallback'ится на user-mode.
- Для обычных scopes вроде `%USERPROFILE%` или `D:\Personal` используется user-mode scanner: так `.gitignore`, hidden и user patterns применяются предсказуемо.

## 5. Phase 1: UI

В Kepler shell (`shell/src/` где у нас Settings views) добавить раздел **«Поиск файлов»**:

- **Папки поиска** — плоский список path'ов + `+` (folder picker через native Electron `dialog.showOpenDialog({ properties: ['openDirectory'] })`) + `−` (удалить).
- **Опции** — два toggle:
  - «Учитывать `.gitignore`» (default ON)
  - «Показывать скрытые файлы» (default OFF)
  - «Ускоренный NTFS-режим» (default OFF)
- **Ignore patterns** — список пользовательских patterns + input add/remove.
- **Действия**:
  - «Переиндексировать» — кнопка вручную триггерит `rescan()`. Полезно после добавления нового root'а или для дебага.
  - Опционально: статус «Проиндексировано N файлов, последнее обновление: …» (если просто и быстро — добавляем; если требует API extension — Phase 1.1).

**Без** per-root settings, без disabled-toggle на Phase 2 фичи, без advanced options collapsible. Простота.

## 6. Phase 1: миграция existing users

При обновлении Kepler с pre-Phase-1 версии на Phase 1:

- Существующая `file_index.db` остаётся. SQLite schema additive (`CREATE TABLE IF NOT EXISTS file_index_roots`), не destructive.
- При первом запуске Phase 1: если `file_index_roots` пустая → заполнить дефолтным `%USERPROFILE%`. (Существующие `["C:\\", "D:\\"]` сейчас в коде, не в БД — они hardcoded.)
- Существующий index (если был) переиндексируется автоматически при first `rescan()` после старта — это normal flow, не migration step.
- Backward-compat: `KEPLER_FILE_INDEX_ROOTS` env var продолжает работать как override.

Никаких explicit migration commands. Никаких UI promp'ов «mig'ация началась». Тихо переходит.

## 7. Phase 1: acceptance criteria

PASS только если **все**:

1. После build + установки Phase 1 — `file_index.db` индексирует `%USERPROFILE%` за разумное время (≤30 сек на типичной машине разработчика с ~50k файлов в home, не считая исключённого AppData).
2. Поиск `kepler-backend.exe` находит результат **при условии** что user добавил `D:\Personal` или подобное в Settings → Папки поиска. (Если не добавил — не находит, это expected per single-root default.)
3. Поиск файла внутри `D:\Personal\Hobby\Coding\kosmos\services\kepler-backend\src\file_index\scanner.rs` находит результат после добавления `D:\Personal` в Settings.
4. `node_modules`, `target`, `AppData`, `Cache` **не** появляются в результатах поиска при включённом exclude-noisy.
5. `.git`-папки не появляются в результатах.
6. Hidden files (`.config`, `.ssh` etc.) **не** появляются если toggle OFF.
7. После удаления root через UI и rescan — файлы из этого root исчезают из index.
8. Пользовательский ignore pattern `*.tmp` удаляет matching files из результатов после rescan.
9. NTFS accelerated toggle OFF гарантирует user-mode scan; ON пробует existing NTFS fast path для drive roots и fallback'ится без hard-fail.
10. `cargo test -p kepler-backend file_index::` всё зелёное (включая существующий regression `replace_all_dedupes_duplicate_paths`).
11. `bun run --cwd shell typecheck` зелёное.
12. `bunx playwright test --config shell/playwright.config.ts` — пройти существующие тесты (не сломали).
13. Postmortem entry в `docs-site/agents/postmortems.md` дописан: UNRESOLVED статус заменён на RESOLVED + Phase 2 ссылка.
14. Manual smoke на машине пользователя ≥1 неделя без жалоб.

## 8. Phase 1: scope guards (что НЕ делаем)

**Тестируемые commitments** (если в diff найдено что-то из списка — это сигнал что я нарушил discipline):

- ❌ Файл `services/kepler-backend/src/file_index/scanner/mft.rs` (или подобный)
- ❌ Файл `services/kepler-backend/src/file_index/scanner/usn.rs`
- ❌ Импорт чего-либо требующего admin/SeBackupPrivilege в Phase 1 code
- ❌ Импорт `windows` / `windows-rs` crate для volume-level handles
- ❌ Любой `impl FileScanner` кроме `UserModeScanner`
- ❌ `unimplemented!()` / `todo!()` stub в trait impl'ах
- ❌ Новые operation kinds в `kepler-focus-svc` named-pipe protocol (`services/kepler-focus-svc/` files)
- ❌ Изменения в `services/kepler-focus-helper/`
- ❌ Изменения в installer scripts (`shell/electron-builder.*`, NSIS configs)
- ❌ Disabled / hidden toggle в UI с label содержащим «Быстрый» / «Fast» / «MFT» / «USN»
- ❌ TODO-комментарии в коде содержащие «Phase 2» / «MFT» / «USN» / «service» / «fast scanner» — если нужно зарезервировать что-то для будущего, кладём в этот документ, не в код
- ❌ Изменение role `kepler-focus-svc` от «hosts override для focus mode» — она остаётся узкой

Если ловлюсь на нарушении одного из пунктов в процессе работы — стоп, не делаю, фиксирую в этом документе как «future work» и продолжаю Phase 1 без этого изменения.

## 9. Phase 1: dependencies + commands

Новые dependencies (Rust):

- `ignore = "0.4"` — gitignore handling
- `dirs = "5"` (если ещё не подключён — для `home_dir()` fallback. Проверить `Cargo.toml`.)

Команды для прогона перед коммитом:

```powershell
cargo test -p kepler-backend file_index::
bun run --cwd shell typecheck
bun run ark:guard:writes          # не должен ничего ловить — мы не трогаем ARK
bun run docs:check
bun run docs:sync
```

E2e (опционально, если есть spec покрывающий file search):

```powershell
$env:KOSMOS_HEADLESS = "1"
bunx playwright test --config shell/playwright.config.ts
```

## 10. Pending empirical test (перед Phase 2 spec.md)

**Эмпирика которую пользователь должен сделать (5 минут):**

1. Открыть Raycast → Settings → File Search → Search Scopes → нажать `+` → добавить `D:\` (целиком).
2. Подождать пока Raycast начнёт что-то индексировать.
3. Попробовать найти заведомо глубокий файл (например, `scanner.rs` или `kepler-backend`).
4. Зафиксировать какой из сценариев происходит:
   - **А)** Медленный user-mode index, через N минут всё работает.
   - **Б)** Контекстный UAC promt «использовать быстрый поиск? нужно подтверждение администратора».
   - **В)** Отказ / progress bar с warning «слишком большой диск, добавьте sub-folders».

От результата зависит UX-pattern Phase 2:

- (А) → Fast Scanner нужен только UX-нетерпеливым. Низкий priority Phase 2.
- (Б) → копируем контекстный pattern (запрос UAC когда indexer заметил что user добавил большой root). Лучше чем явный toggle.
- (В) → Raycast выкатил MFT назад из MSIX-сборки. Может быть и нам не нужен. Pure Phase 1 forever.

## 11. Phase 2: triggers, open questions, NON-spec

**Phase 2 spec.md пишется ТОЛЬКО когда выполнено всё нижеследующее:**

1. Phase 1 закоммичен и зелёный по гвардам.
2. ≥1 неделя использования Phase 1 пользователем без проблем.
3. Пользователь сообщил **конкретный gap** который не покрылся Phase 1 (например, «не нахожу файлы на `D:\Games` потому что добавил его в roots, но scan не доходит за разумное время»).
4. Эмпирика Raycast `D:\` поведения (§ 10) сделана.

Если за неделю нет конкретного gap — Phase 2 откладывается дальше или отменяется. Это нормальный исход.

**Open questions для будущего spec.md** (не решаем сейчас, только фиксируем):

### 11.1 Service architecture

Reuse `kepler-focus-svc` vs новый `kepler-index-svc`.

**Lean — отдельный `kepler-index-svc`.** Аргументы:

- Naming honesty — focus-svc про hosts override, не file system
- Privilege scope discipline — least privilege per component
- Audit story короче на каждый сервис
- Failure isolation — bug в focus mode не валит file search

Цена «двух служб в services.msc» — минорная.

### 11.2 Install timing

Eager (оба сервиса при Kepler install, stopped/manual) vs lazy (index-svc устанавливается при первом toggle в Settings).

**Lean — lazy.** UAC появляется в момент когда понятно зачем (пользователь явно включил «Быстрый поиск»).

### 11.3 Scan technology

USN journal vs MFT direct — **связка, не альтернатива**:

- Bootstrap через MFT direct (быстрый initial enumeration)
- Watch через USN journal tail (incremental change events)
- Оба требуют volume handle = admin / `SeBackupPrivilege`
- `ReadDirectoryChangesW` (user-mode) не подходит — lossy под high event load, не volume-wide

### 11.4 Concurrency между service и backend

**Option 1: shared SQLite с ACL.** Service-SYSTEM пишет, backend-user читает. WAL mode reader+writer concurrency. Сложность: SYSTEM-владение долгоживущим shared resource, ACL setup, потенциально WAL frame writes из reader.

**Option 2: pipe stream → user-mode SQLite.** Service compute-only, стримит результаты в backend по pipe, backend владеет DB. Bootstrap ~500MB по pipe (~1-2 сек на TB-диск). Incremental USN events tiny.

**Option 3 (rejected):** Service держит in-memory index, IPC на каждый query. Wasteful, state loss on crash, IPC roundtrip latency. Не рассматривается.

**Lean — Option 2** за privilege-boundary cleanness. Но это spec.md decision на основании benchmarks.

### 11.5 UAC trigger pattern

Опции (зависит от эмпирики § 10):

- Явный toggle в Settings → File Search → «Быстрый поиск (требует admin)» (legacy Raycast v0.29 паттерн)
- Contextual prompt когда indexer заметил `>N files` или `>M sec` (вариант Б из эмпирики)
- Hybrid — toggle в Settings + контекстный hint banner когда видим slow scan

### 11.6 Migration story

Что происходит при обновлении Kepler с Phase 1 на Phase 2:

- Default state нового toggle (предположительно OFF — safest)
- Существующий user-mode index — preserved или rebuilt?
- Если service install fails / user refuses UAC — graceful degrade на Phase 1 logic

### 11.7 Failure modes

Service crashed / disabled / uninstalled — fallback semantics. Должно быть **honest degradation** (не silent slow scan):

- UI banner «Быстрый поиск недоступен, индекс на пользовательском уровне»
- Search всё равно работает через `UserModeScanner` как fallback

### 11.8 Uninstall semantics

Когда user удаляет Kepler — что с `kepler-index-svc`? Когда выключает toggle в Settings — service uninstalls или просто stops?

### 11.9 Testing strategy

E2e с реальным UAC elevation — almost certainly непрактично (как было с autostart, см. postmortems.md). Realistic: unit + integration tests против mock service. Manual repro для full flow.

## 12. Order of operations

1. **Phase 1 implementation** (~полдня focused work):
   - Refactor scanner.rs под trait
   - Заменить default roots
   - Подключить `ignore` crate
   - Добавить settings keys + roots table
   - Settings UI «Папки поиска»
   - Регрешн-тесты
   - Postmortem entry
   - Один commit на всё (с описанием, ссылающимся на этот документ)

2. **Smoke + ship**:
   - Прогон guard'ов
   - Manual smoke на машине пользователя (добавить `D:\Personal`, найти `scanner.rs`)
   - Если всё ок — оставить работать ≥1 неделя без других изменений в file_index

3. **Параллельно — эмпирика Raycast** (5 минут, см. § 10):
   - User добавляет `D:\` в Raycast scopes
   - Фиксирует поведение
   - Дописывает результат в этот документ § 10

4. **После ≥1 недели**:
   - Если нет конкретных gap — Phase 2 откладывается / отменяется
   - Если есть gap + есть эмпирика Raycast → пишем `.agent/tasks/<DATE>-fileindex-fast-scanner/spec.md`

5. **Phase 2 implementation** — как отдельный focused work block, не вперемешку с другими задачами.

## 13. References

- Bug origin: `docs-site/agents/postmortems.md` § 2026-05-23 «Kepler: file search показывает только %APPDATA% (UNRESOLVED)»
- Текущий код file_index: `services/kepler-backend/src/file_index/{mod.rs,scanner.rs,store.rs,watcher.rs,scanner/ntfs.rs}`
- Существующий регрешн: `services/kepler-backend/src/file_index/store.rs::tests::replace_all_dedupes_duplicate_paths`
- Memory: `feedback_epistemic_discipline.md` (rule про uncertainty markers), `project_raycast_file_search_two_modes.md` (фактическая картина Raycast)
- Раycast technical blog: https://www.raycast.com/blog/a-technical-deep-dive-into-the-new-raycast
- `ignore` crate: https://github.com/BurntSushi/ripgrep/tree/master/crates/ignore
- Связанные skills: `.agents/skills/bug-postmortem/SKILL.md`
- Эмпирические скриншоты Raycast Settings — у пользователя (не в репо)

---

**Owner:** этот документ — source of truth. Изменения архитектуры или scope — сначала PR на этот файл, потом код. Любой gap между документом и кодом = проблема в коде или в документе, не «допустимый рассинхрон».
