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

## 🟡 Pending — Arrancador full completion (2026-05-18)

См. `.agent/tasks/2026-05-18-arrancador-full-completion/spec.md` AC11.

**Подготовка:**

- На машине установлены: **Dota 2** (Steam 570), **Cairn** (Steam, The Game Bakers), **Outlast** (Steam 238320). Желательно все три, минимум — две.
- Steam library в стандартном `C:\Program Files (x86)\Steam\` либо в кастомном пути (отметить в `arrancador-config.json` через UI Settings → `custom_scan_paths` — TODO, в MVP пока только default path).
- RAWG API ключ — получить на https://rawg.io/apidocs если нет.

**Старт:**

```powershell
bun run --cwd shell dev
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
- [ ] **Console errors.** F12 в Arrancador window → Console. Никаких «Unknown operation arrancador.*» или unhandled rejections.

**Если что-то падает** — создай `problems.md` в proof loop'е соответствующей задачи и опиши проблему.

---

## 🟡 Pending — Phase 7 universal export (2026-05-18)

См. `.agent/tasks/2026-05-18-phase-7-universal-export/spec.md` AC11 / smoke раздел.

**Подготовка:** в ARK должны быть `note_obj` (Eden создавал), `task_obj` (Delphi), `tag_obj`, `game_obj` (Arrancador). Если каких-то типов нет — те конвертеры просто отработают на пустом списке (создадут пустой CSV / JSON).

**Старт:** `bun run --cwd shell dev` → Kepler → Settings → таб «Экспорт».

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

- [ ] `bun run --cwd shell dev` → Ctrl+Shift+K → «Открыть Eden».
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

- [ ] Fresh checkout / новая Windows-сессия → `bun install && bun run --cwd shell build:js && bun run test:e2e` → **без admin'а**. Все 20 e2e зелёные, никаких `EPERM` на `tests/.e2e/<slug>/kepler.lock.json`.
- [ ] Lock-файлы в `tests/.e2e/` после прогона можно открыть/удалить обычным user'ом (никакого `icacls /reset` не нужно).
- [ ] В prod (НЕ выставлен `KOSMOS_LOCK_PERMISSIONS_DISABLED`) lock остаётся с жёстким ACL — `icacls %APPDATA%\Kosmos\kepler.lock.json` показывает только текущий user.

---

## ✅ Закрытые

(перенеси сюда пункты после прохождения — с датой и комментариями если важно)

- _пусто пока_

---

## Связанные

- [Proof loop](/concepts/proof-loop)
- [Testing](/agents/testing)
- [Estimation](/agents/estimation)
