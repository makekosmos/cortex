# Manual tests waiting

::: tip Этот файл — твой TODO-чек-лист
Здесь висят пункты, которые `cargo test` / `playwright` / `typecheck` / `build` не покрывают — нужна **визуальная / интерактивная** проверка человеком. Когда пункт пройден — сними галочку и удали запись (или помечай дату закрытия в evidence соответствующего proof loop'а).
:::

## Как пользоваться

- Каждый пункт ссылается на свой proof loop в `.agent/tasks/<DATE>-<slug>/`.
- Команды запускать в PowerShell **из репо**.
- Headless mode тестов **не** активируется, потому что пользователь должен видеть UI.
- Если что-то не работает — добавь в `problems.md` соответствующего proof loop'а и пингай агента для fix'а.

---

## 🟠 Pending — macOS hotkey capture (2026-06-07)

Заполнен variation point macOS hotkey capture (Swift helper `capture-hotkey` + adapter). Proof loop — `.agent/tasks/2026-06-07-macos-hotkey-capture-adapter/`.

**Helper-уровень проверен автоматически на живом маке (2026-06-07)** через CGEvent-инъекцию (`capture-driver`, `.tmp/native-tests/`):

- [x] event tap создаётся (`{"ready":true}`) — Input Monitoring permission ок.
- [x] `Cmd+Shift+H` → `captured keyCode=4 cmd shift` (letter).
- [x] `Ctrl+Alt+;` → `captured keyCode=41 ctrl alt` (symbol).
- [x] `Escape` → `cancelled reason=escape`.
- [x] пассивно (без нажатий) helper не эмитит фантомных `captured`.

> No-modifier ignore инъекцией не проверяется надёжно: CGEventPost с `maskCommand` загрязняет глобальное состояние модификаторов и плодит эхо-события. Фильтр `if !(cmd||ctrl||alt||shift)` тривиален и виден в коде helper'а.

**Осталось ручное (UI-уровень):** `bun run --cwd platform/desktop dev` на macOS:

- [ ] Settings → Диктация → «Горячая клавиша» → клик → поле показывает «Нажми сочетание…».
- [ ] Зажать `Cmd+Shift+;` — поле принимает сочетание (`⌘ ⇧ ;`), конфиг сохраняется (через `pendingAccelerator`).
- [ ] `Escape` во время capture — отмена, поле возвращает прежнее значение.
- [ ] Без Input Monitoring permission — capture не виснет (адаптер шлёт `cancelled`), UI выходит из ожидания.
- [ ] Назначенный хоткей реально триггерит диктацию (hold-monitor подхватывает новый accelerator).

---

## 🟠 Pending — Settings sidebar + command visibility (2026-05-23, Kepler 0.2.8)

Полная переработка Settings UI: sidebar навигация, поиск, страницы с `SettingsAdvancedIntro`, tray-toggle и управление видимостью команд.

**Старт:** `bun run --cwd platform/desktop dev`

### Чек-лист

#### Sidebar и навигация

- [ ] Settings окно открывается (`tray → Настройки`). Слева — sidebar 229px с заголовком «Настройки», полем поиска и двумя группами кнопок: **Общие / О приложении / Дебаг** (верхняя) и **Заметки / Задачи / Времяметр / Игры / Фокус / Расширения / Поиск файлов** (нижняя).
- [ ] Нажатие на любую кнопку в sidebar открывает соответствующую страницу. Активная кнопка подсвечена фоном `#343434`.
- [ ] Поиск в sidebar фильтрует кнопки по keywords. `«трей»` → остаётся только «Общие». `«eden»` → остаются Заметки + Расширения. При пустом query — все кнопки.
- [ ] Поиск, не дающий результатов, показывает «Ничего не найдено» в sidebar и пустой контент.

#### Общие

- [ ] Страница «Общие» содержит хоткей, автозапуск, **«Показывать в трее»** toggle и «Запоминать позицию в лаунчере».
- [ ] Toggle «Показывать в трее» → OFF → иконка в трее исчезает без перезапуска. Повторно → ON → значок возвращается.

#### О приложении

- [ ] Страница «О приложении» содержит версию Kepler и кнопку «Проверить обновления».

#### Дебаг

- [ ] Страница «Дебаг» содержит Developer mode, Usage tracker, позицию в лаунчере, Backend status, Отчёты об ошибках, Bug-report.

#### Поиск файлов

- [ ] Страница «Поиск файлов» содержит toggle «Исключать шумные папки».

#### Управление командами (Заметки / Задачи / Времяметр / Игры)

- [ ] Страница «Заметки» содержит список команд Eden с чекбоксами. Снять галочку с «Открыть Eden» → команда исчезает из launcher'а (проверить `Alt+Space`).
- [ ] Переоткрыть Settings — снятая галочка сохранилась.
- [ ] Включить команду обратно → появляется в launcher'е.
- [ ] Раздел «Времяметр» дополнительно содержит toggle «Трекать активные приложения».

#### Window chrome

- [ ] Settings окно имеет нативный titlebar overlay (кнопки закрытия, свернуть) высотой 36px, полупрозрачный фон (acrylic).
- [ ] Sidebar title «Настройки» — drag-area: окно можно перетащить за него.

---

## 🟠 Pending — Focus mode end-to-end (2026-05-18, Kepler 0.2.4+)

Полная реализация focus mode shipped: helper bin + ARK schema + Settings UI +

Тесты ниже **требуют admin rights** на машине (helper.exe запускается с UAC).

### Prep

1. Установи `Kepler Setup 0.1.18.exe` (или новее).
2. Backup своего `C:\Windows\System32\drivers\etc\hosts` (на всякий — `copy hosts hosts.user-backup`).
3. Открой Settings (tray icon → Настройки) → видишь tab **«Фокус»**.

### Чек-лист

#### Базовый flow

- [ ] Settings → Фокус → «Создать блок-лист» → name «Test», textarea:
  ```
  tiktok.com
  twitter.com
  # этот сайт сейчас не блокируется
  reddit.com
  ```
  → нажми «Сохранить». В списке появилась запись «Test · 3 домена».
- [ ] Нажми «Включить» на «Test» → **UAC prompt** появляется → одобри → запись «Активна» появилась в верхней секции.
- [ ] Открой `C:\Windows\System32\drivers\etc\hosts` в Блокноте (admin) → видишь блок:
  ```
  # === kepler-focus BEGIN ===
  127.0.0.1 tiktok.com
  127.0.0.1 www.tiktok.com
  127.0.0.1 twitter.com
  127.0.0.1 www.twitter.com
  127.0.0.1 reddit.com
  127.0.0.1 www.reddit.com
  # === kepler-focus END ===
  ```
- [ ] Также появился `hosts.kepler-backup` — это backup оригинала.
- [ ] Открой браузер → https://tiktok.com → **ERR_CONNECTION_REFUSED / Connection refused** (mini HTTP server для friendly block page — на v2). Это ОК — блокировка работает.
- [ ] Settings → Фокус → «Отключить» → UAC снова (или может cached) → одобри → hosts очищен от kepler-section.
- [ ] Перезагрузи браузер DNS cache (`Ctrl+Shift+Delete` или просто перезапусти браузер) → tiktok.com снова открывается.

- [ ] Справа от input → 🛡️ chip → выбери «Test» blocklist.
- [ ] Старт pomodoro → UAC prompt → одобри.
- [ ] Focus widget (320×52, всегда сверху) показывает 🛡️ слева от MM:SS.
- [ ] hosts модифицирован, tiktok.com заблокирован.
- [ ] Stop pomodoro → блокировка автоматически снимается, 🛡️ исчезает с widget'а.

#### Кейсы edge

- [ ] **Перезапуск Kepler во время активной блокировки.** Активируй blocklist → закрой Kepler (tray → Quit). hosts остаётся модифицирован (известная limitation). Запусти Kepler снова → Settings → Фокус показывает active state → можно «Отключить» → hosts очищен.
- [ ] **Uninstall Kepler с активной блокировкой.** Активируй blocklist → запусти uninstaller (`Программы и компоненты → Kepler → Удалить`) → во время uninstall NSIS hook запускает helper reset → hosts очищен. Проверь hosts после uninstall — должен быть без kepler-section.
- [ ] **Re-install после uninstall.** Поставь Kepler 0.1.18 снова → создай blocklist → активируй → UAC появляется → работает корректно.

#### Edge-case: невалидные домены

- [ ] Settings → Фокус → «Создать блок-лист» → textarea: `not-a-valid-thing` + `tiktok.com` → видишь label «Валидных: 1 · невалидных: 1». Save — сохраняется только `tiktok.com`.
- [ ] Drag&drop .txt файла с доменами в textarea → текст подставлен.

### Если что-то не работает

- **UAC не появляется** → Kepler возможно уже запущен от админа (тогда helper spawn'ится direct без UAC). Проверь Task Manager → Kepler.exe → User name column.
- **hosts не меняется после UAC одобрения** → проверь `kepler-focus-helper.exe` существует в `%LOCALAPPDATA%\Programs\Kepler\resources\`. Если нет — переустанови.
- **PowerShell error в logs** → открой Kepler DevTools (`F12` в launcher если включен dev mode), смотри console → `[focus-block] helper failed: <reason>`. Часто это «User cancelled UAC» (норм) или path issue.

---

## 🟡 Pending — Arrancador full completion (2026-05-18)

См. `.agent/tasks/2026-05-18-arrancador-full-completion/spec.md` AC11.

**Подготовка:**

- На машине установлены: **Dota 2** (Steam 570), **Cairn** (Steam, The Game Bakers), **Outlast** (Steam 238320). Желательно все три, минимум — две.
- Steam library в стандартном `C:\Program Files (x86)\Steam\` либо в кастомном пути (отметить в `arrancador-config.json` через UI Settings → `custom_scan_paths` — TODO, в MVP пока только default path).
- RAWG API ключ — получить на https://rawg.io/apidocs если нет.

**Старт:**

```powershell
bun run --cwd platform/desktop dev
```

В launcher (Ctrl+Shift+K) → «Открыть Arrancador».

**Чек-лист:**

- [ ] **Scanner.** Открой ScanPage → нажми «Сканировать сейчас». Должно появиться: «Добавлено: ≥2 (Dota 2 / Cairn / Outlast)» либо «Обновлено» если уже были. В history (last 10) запись с timestamp + counts.
- [ ] **Library.** LibraryPage показывает все три игры с обложками (если RAWG ещё не применён — может быть placeholder image). Поиск работает по имени.
- [ ] **Launch.** Нажми «Запустить» на Dota 2 → Steam launcher должен открыться и запустить игру. Через минуту закрой её. Проверь Statistics — `usage_session` должна была быть записана (через `usage_tracker` модуль).
- [ ] **RAWG search.** В Settings задай RAWG API key → Save. В Catalogue введи «Dota 2» → 500ms debounce → появятся результаты. Click «Применить metadata к» → выбери Dota 2 из dropdown → Apply → success toast. В Library у Dota 2 теперь обложка / описание из RAWG.
- [ ] **RAWG для Cairn.** Search «Cairn The Game Bakers» в Catalogue → найди → apply → проверь metadata в Library.
- [ ] **SQOBA backup.** В SqobaPage разверни Dota 2 → «Создать бекап». Должен создать zip в `%APPDATA%\Kosmos\sqoba\<game_id>\<timestamp>.zip`. Если save paths не найдены — error «No save paths found» (это OK для Dota 2 — сейвы Steam Cloud, не локально).
- [ ] **SQOBA backup для Outlast.** Outlast хранит save в `%LOCALAPPDATA%\OLG\Saved\` — должен найтись и забекапиться. Создай бекап → проверь файл.
- [ ] **SQOBA restore.** Удали один файл из сейв-папки Outlast → SqobaPage → выбери backup → «Восстановить» → Modal-confirmation → confirm → файл должен вернуться.
- [ ] **SQOBA rotation.** Сделай 12 backup'ов Outlast подряд (тестово). После 11-го самый старый должен удалиться (rotation keep N=10).
- [ ] **Console errors.** F12 в Arrancador window → Console. Никаких «Unknown operation arrancador.\*» или unhandled rejections.

**Если что-то падает** — создай `problems.md` в proof loop'е соответствующей задачи и опиши проблему.

---

## 🟡 Pending — Phase 7 universal export (2026-05-18)

См. `.agent/tasks/2026-05-18-phase-7-universal-export/spec.md` AC11 / smoke раздел.

**Подготовка:** в ARK должны быть `note_obj` (Eden создавал), `task_obj` (Delphi), `tag_obj`, `game_obj` (Arrancador). Если каких-то типов нет — те конвертеры просто отработают на пустом списке (создадут пустой CSV / JSON).

**Старт:** `bun run --cwd platform/desktop dev` → Kepler → Settings → таб «Экспорт».

**Чек-лист:**

- [ ] **Список конвертеров.** Видишь 6 карточек: «Заметки → Markdown», «Задачи → Markdown», «Задачи → CSV», «Time entries → CSV», «Tags → JSON», «Games → JSON».
- [ ] **Export note_obj → md.** Нажми «Экспортировать» на «Заметки → Markdown» → выбери папку `D:\tmp\eden-export` (создай если нет). Каждая заметка → `<title>.md` с YAML frontmatter (id, type, title, created/updated, header_props) + markdown body из TipTap.
- [ ] **Frontmatter валидный.** Открой один .md в editor / Obsidian → frontmatter парсится, видно поля.
- [ ] **Body markdown.** Параграфы / heading / lists / code_block / blockquote / marks (bold, italic, code, link) корректно сконвертированы.
- [ ] **Export task_obj → md.** То же для задач. Body должен иметь `## Чек-лист` с `- [ ] / - [x]`.
- [ ] **Export task_obj → csv.** RFC 4180 CSV: один `tasks.csv`, колонки id/title/project_id/area_id/scheduled_date/deadline/completed/priority/tags (tags через `|`). Открой в Excel — корректные колонки.
- [ ] **Export time_entry_obj → csv.** `time-entries.csv` с колонками id/title/started_at/ended_at/duration_minutes/task_id/source.
- [ ] **Export tag_obj → json.** `tags.json` — pretty-printed массив.
- [ ] **Export game_obj → json.** `games.json` — массив с игр.
- [ ] **History.** В UI «Последние экспорты» (10 шт в localStorage) — запись каждого export'а с timestamp, dest_dir, count, ok-флагом.
- [ ] **Filename collision.** Если 2 заметки с одинаковым title — второй файл получит суффикс `-2.md`. Создай вторую заметку с тем же title → export → проверь.
- [ ] **Errors.** Если папка read-only / нет прав → exports `errors[]` непустой, UI показывает текст ошибки.

---

## 🟡 Pending — Eden Phase 6.0 (2026-05-17)

См. `.agent/tasks/2026-05-17-eden-extension/spec.md` AC10.

**Что проверить:**

- [ ] `bun run --cwd platform/desktop dev` → Ctrl+Shift+K → «Открыть Eden».
- [ ] Окно Eden открывается размер ~1100×750. TipTap editor виден на main view.
- [ ] Список заметок (если есть `note_obj` в ARK) рендерится в сайдбаре.
- [ ] Создать новую заметку (`+` button) → набрать текст → закрыть Eden → переоткрыть → заметка persisted.
- [ ] Корзина (Settings → Корзина) — удалённые заметки видны, restore / permanent delete работает.
- [ ] Editor.vue lazy chunk: DevTools → Network → при открытии первой заметки подгружается `Editor-*.js` (~1.36MB).
- [ ] Commands `eden:note:create` / `eden:note:search` доступны в launcher и работают.

---

## 🟡 Pending — Lock-file test isolation (2026-05-17)

См. `.agent/tasks/2026-05-17-lock-file-test-isolation/spec.md` AC8.

**Что проверить:**

- [ ] Fresh checkout / новая Windows-сессия → `bun install && bun run --cwd platform/desktop build:js && bun run test:e2e` → **без admin'а**. Все 20 e2e зелёные, никаких `EPERM` на `tests/.e2e/<slug>/kepler.lock.json`.
- [ ] Lock-файлы в `tests/.e2e/` после прогона можно открыть/удалить обычным user'ом (никакого `icacls /reset` не нужно).
- [ ] В prod (НЕ выставлен `KOSMOS_LOCK_PERMISSIONS_DISABLED`) lock остаётся с жёстким ACL — `icacls %APPDATA%\Kosmos\kepler.lock.json` показывает только текущий user.

---

## 🟡 Pending — EXPTOTRY batch experiments (2026-05-18)

См. `EXPTOTRY.md` + `.agent/tasks/2026-05-18-exp08-electron-languages/` + следующие proof loop'ы.

### Exp 08 — `electronLanguages` (применён, нужен GUI smoke)

После переустановки `release/Kepler Setup 0.1.9.exe`:

- [ ] **Запуск.** Установка проходит, Kepler стартует с tray icon, главное окно (Ctrl+Shift+K) показывается, acrylic фон на месте.
- [ ] **Settings.** Tray menu → «Настройки» → окно открывается, все табы рендерятся, нативные диалоги (filepicker в табе «Экспорт») показывают строки без тарабарщины (en-US fallback для не-ru систем).
- [ ] **Context menu.** ПКМ в Eden editor / Delphi textarea → нативное контекстное меню Chromium показывает читаемые подписи (Cut/Copy/Paste либо локализованные).

Если что-то сломано — откатить: убрать строку `"electronLanguages": [...]` из `platform/desktop/package.json` build блока.

### Exp 7 — explicit `backgroundThrottling: true` (применён, проверка нерегрессии)

Применено к settings/install-extension/dashboard окнам. Defensive (default уже true в Electron).

- [ ] **Settings window background behavior.** Открой Settings → переключи фокус на другое окно → проверь, что Settings не тормозит при возврате (default throttling ожидаемо).
- [ ] **Dashboard background behavior.** Открой Dashboard (tray menu) → фокус на другое окно → возврат не должен показать визуальные глитчи / stale данные.

### Exp 30 — CSS `contain: layout style` на extension roots (применён, проверка нерегрессии)

- [ ] **Arrancador.** Library scroll, переключение pages — нет clipped overflow.
- [ ] **Delphi.** Sidebar + main pane, drag&drop задач, modal'ы (QuickEntry / QuickOpen) — нет визуальных регрессий.
- [ ] **Eden.** Editor + sidebar resize, modal overlays (search, settings) — нет clipped content.

CSS `contain: layout style` изолирует reflow scope, но не paint scope (`contain: paint` мог бы обрезать тени / outline'ы — поэтому не выставлен).

### Exp 39 — TS `incremental` (применён, не требует UI smoke)

Cold typecheck 1650ms → warm 1177ms (−28%). Effect для DX в watch-mode.

---

## 🔴 Tech debt — Export tab disabled (2026-05-18)

**Симптом:** На production install 0.1.11 страница «Экспорт» падает на whitescreen / catch-all error. Tab временно закомментирован в `platform/desktop/src/views/SettingsView.vue` (tab nav + content), функциональность доступна через WS API напрямую но не через UI.

**Что уже сделано:**

- 0.1.10 fix: `main.ts` IPC handler `kepler:export:list` unwraps `{converters: [...]}` → array (backend returns wrapped object). Это починило "e is not iterable" в loadExportConverters().
- 0.1.11 не помог по user-репорту — есть ещё какая-то ошибка в renderer'е страницы.

**Что нужно отдебажить:**

- Запустить production build, открыть DevTools (F12 if not blocked), переключиться на Экспорт → смотреть console errors.
- Возможные причины:
  - `c.supported_formats` undefined (template `v-for in c.supported_formats`) — если backend converters почему-то имеют разные shape.
  - `exportSelectedFormat[c.converter_id]` undefined при первой загрузке.
  - `exportHistory` malformed в localStorage от прошлых версий.
- Hardening: добавить `v-if="c.supported_formats && c.supported_formats.length > 0"` guard'ы; safe-default для exportSelectedFormat.

**Когда чинить:** после того как user сообщит что extensions marketplace работает (current priority). Возвращаем Экспорт tab + правим renderer crash.

См. proof loop задачи `2026-05-18-export-bug-tech-debt` — будет создана при возврате к восстановлению Экспорт tab.

---

## ✅ DONE — RAM baseline (2026-05-18)

Собран agent'ом через production build + isolated data dir. См. `.agent/tasks/2026-05-18-ram-baseline-harness/baseline-results.md`.

| Scenario                                | Private bytes (mean) | Verdict                               |
| --------------------------------------- | -------------------: | ------------------------------------- |
| launcher-only                           |             235.3 MB | baseline                              |
| all-extensions-idle (5 ext + dashboard) |         **453.5 MB** | ✅ Exp 5 правильно deferred (<600 MB) |
| exp23-acrylic                           |             291.2 MB | —                                     |
| exp23-mica                              |             305.4 MB | RAM Δ ~5%, в пределах шума            |

**Остаётся manual (GPU% при visible launcher):** Exp 23 final decision. См. блок «Exp 23 — Mica» ниже.

---

## ⏸️ Pending — RAM baseline collection (historical instructions)

**Это первый блокер для всех defer-experiments.** Без baseline все «применим если будет signal» бессмысленны — никто никогда не соберёт data без явного запуска.

### Цель

Собрать numeric baseline для 4 сценариев — это даёт **trigger thresholds** для defer-experiments (Exp 4, 5, 23, 27, 50).

### Подготовка

- Закрой все апки кроме браузера + терминал (минимальная фоновая нагрузка).
- Power plan: «Балансированный» или выше (не Power Saver — он throttles CPU/GPU).
- Один монитор активен (multi-display добавляет DWM overhead).

### Запуск

```powershell
pwsh scripts/run-baseline-scenarios.ps1
```

Orchestrator проведёт через 4 сценария:

#### Scenario 1: launcher-only

```powershell
bun run --cwd platform/desktop dev
# дождись когда launcher открылся (Ctrl+Shift+K, увидел launcher окно).
# нажми Enter в orchestrator. Жди 30s warmup + 30s samples.
```

#### Scenario 2: all-extensions-idle

```powershell
$env:KEPLER_BENCHMARK_OPEN_ALL = "1"
bun run --cwd platform/desktop dev
# Подожди 30s чтобы окна полностью загрузились + 5min idle для стабилизации памяти.
# Нажми Enter в orchestrator (warmup 30s ещё подождёт).
```

#### Scenario 3+4: Exp 23 A/B (Mica vs Acrylic)

```powershell
# Сначала acrylic вариант:
$env:KEPLER_BG_MATERIAL = "acrylic"; bun run --cwd platform/desktop dev
# Открой Task Manager → Performance → GPU. Засеки:
#   - dwm.exe %GPU
#   - kepler-shell.exe %GPU
# Запиши на бумажку. Нажми Enter в orchestrator.

# Закрой Kepler. Перезапусти с mica:
$env:KEPLER_BG_MATERIAL = "mica"; bun run --cwd platform/desktop dev
# Снова Task Manager → запиши те же метрики.
# Сравни.
```

### Чек-лист после baseline

- [ ] 4 JSON отчёта в `.tmp/ram-kepler-<scenario>-<ts>.json`.
- [ ] Зафиксированы цифры dwm.exe %GPU для acrylic vs mica (Task Manager).
- [ ] Перенесены summary в `docs-site/concepts/ram-benchmarks.md` (раздел «Trigger thresholds»).

### Decision tree

После сбора:

```
all-extensions-idle.totals.mean_private_mb:
  > 900 MB  → 🚨 critical, applied Exp 5 (WebContentsView) сейчас
  600-900   → ⚠️  применять Exp 5 в следующий sprint
  < 600     → ✅ OK, Exp 5 остаётся deferred

exp23-acrylic dwm.exe %GPU sustained:
  > 15%     → переключиться на Mica (Exp 23 apply)
  10-15%    → marginal, выбор по эстетике
  < 10%     → оставить Acrylic
```

---

## 🔵 Ready-to-apply (требуют GUI verification ПЕРЕД применением)

### Exp 46 — `shallowRef` для Delphi todos store

**Statics:** semantic анализ показал безопасность (`updateTodo` reassigns array; нет прямых `todos.value[i].field = x` мутаций). Win: меньше Proxy overhead для больших списков задач.

Файл: `products/delphi/src/store/todos.ts` — `todos = ref<TodoItem[]>([])` → `shallowRef<TodoItem[]>([])`. Аналогично projects/areas/tags/headings.

**Что нужно проверить перед commit'ом:**

- [ ] Создание/редактирование/удаление задачи в TodayPage / AllTaskPage обновляет UI немедленно.
- [ ] Drag-and-drop порядка задач сохраняется и виден в UI.
- [ ] Markdown task content / tags / due date — реактивные изменения видны без force-refresh.
- [ ] Logbook (completed tasks) обновляется при completion.
- [ ] Project sidebar обновляется при создании / переименовании проекта.

Если хоть один пункт не отрисовывается — откат, либо переход на triggerRef + manual reactivity.

### Exp 27 — виртуализация списков (Delphi / Dashboard)

Не применять, пока нет реальных списков **>500 items** у юзера. До этого момента — net loss (overhead > gain). Триггер: пользователь сообщил, что в Delphi >300 задач или Dashboard >500 objects тормозит scroll.

При имплементации — `@vueuse/core` `useVirtualList` + замена `<TodoRow v-for>` на virtualized container.

### ~~Exp 23 — Mica vs Acrylic~~ ✅ DONE 2026-05-18, обновлено 2026-05-23

Принято решение использовать **Mica** как default backdrop для launcher'а. Settings window переключён на **Acrylic** (2026-05-23) — Mica плохо выглядит с native titleBarOverlay.

- `platform/desktop/electron/main.ts` → `resolveLauncherBgMaterial()` default = `"mica"`.
- `platform/desktop/electron/settings-window.ts` → `"acrylic"` (Acrylic + native titleBarOverlay 36px).
- `platform/desktop/electron/install-extension-window.ts` → `"mica"`.
- Dashboard оставлен solid (frame + titleBarOverlay — Mica с overlay'ем выглядит странно).
- Env override `KEPLER_BG_MATERIAL=acrylic|mica|none` доступен (только для launcher).

### Exp 4 — Hide/Show window pool для extensions

**Архитектурный.** Сейчас extension-host создаёт BrowserWindow при openExtension() и уничтожает при close. Pool бы держал hidden window'у per extension, переоткрытие — `show()` вместо нового spawn.

Trade-off: 70-80% быстрее переоткрытие vs 100-150 MB per hidden window. Для 4 extensions = ~400-600 MB постоянно занятых. Net loss если юзер редко переоткрывает.

Не применять без четкого user signal (например: «extension'ы открываются заметно медленно»).

### Exp 5 — `WebContentsView` migration (WHOLE архитектура)

**Большая работа (1-2 недели).** Эффект: −30% RAM на extension за счёт shared GPU process. Требует:

- Замена BrowserWindow per extension на BaseWindow + WebContentsView внутри Kepler shell.
- Manual управление z-index / bounds / визуальной целостности.
- Пересмотр extension-host'а целиком.

Не приступать без чёткого RAM-bottleneck signal в продакшене.

---

## ✅ Закрытые

(перенеси сюда пункты после прохождения — с датой и комментариями если важно)

- _пусто пока_

---

## Связанные

- [Proof loop](/concepts/proof-loop)
- [Testing](/agents/testing)
- [Estimation](/agents/estimation)
