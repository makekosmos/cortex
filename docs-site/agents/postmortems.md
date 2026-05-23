# Постмортемы багов

::: tip Зачем эта страница
Журнал реальных багов, которые уже починили. Каждая запись = **что сломалось**, **почему**, **как починили**, **как не допустить повторения**. Цель — чтобы будущий ты (или следующий агент) не повторял одни и те же ошибки.

Workflow ведения постмортемов — `bug-postmortem` skill (`.claude/skills/bug-postmortem/SKILL.md`).
:::

## Шаблон записи

```
## YYYY-MM-DD — короткое название

**Симптомы** — что наблюдал пользователь (1-2 предложения).
**Где жило** — `file:line` ссылки.
**Root cause** — почему именно это произошло, на уровне «не угадаешь без stack trace».
**Fix** — что изменили + commit hash (когда появится).
**Регрешн-защита** — какой тест ловит повторение.
**Prevention** — общий вывод: на что обращать внимание в похожих ситуациях.
```

---

## 2026-05-23 — file_index UNIQUE constraint вешает backend

**Симптомы.** После часа работы Kepler внезапно подвисает: Eden показывает infinite loading, `commands.list` / `app_index.list_all` / `focus.*` все таймаутят через 30s. Backend.exe жив как процесс, supervisor не делает respawn, но WS-server не отвечает.

**Где жило.** `services/kepler-backend/src/file_index/store.rs::replace_all` (схема `files.path TEXT PRIMARY KEY` в `store.rs:14`). Вызов из `services/kepler-backend/src/file_index/mod.rs::rescan:102`, async-spawn из `services/kepler-backend/src/main.rs:299`.

**Root cause.** `replace_all` использовал простой `INSERT INTO files (path, ...) VALUES (?, ?, ?, ?)` без дедупа и без `OR REPLACE`. NTFS fast scan на C:\ + D:\ может вернуть один и тот же абсолютный path дважды:

- Junction points (`C:\Documents and Settings` → `C:\Users`)
- Symlinks (часто в `C:\ProgramData\Package Cache`)
- Mount points (D:\ как junction внутри C:\)
- WSL'овский `\\?\` namespace, видимый из двух root'ов

Первый дубликат → `UNIQUE constraint failed: files.path` → транзакция rollback → `Err` из `rescan` → лог пишет `WARN file_index initial rescan failed` (`main.rs:307`) и task завершается. **Сам fail не должен валить backend** — но в наблюдаемом инциденте после WARN backend-лог обрывается на час, потом начинают сыпаться 30s-таймауты на всех ark-операциях. Точная second-order причина (Windows hibernate vs. блокировка tokio worker через `std::sync::Mutex<Connection>` в долгой транзакции vs. шторм событий watcher'а от browser hang) не установлена однозначно из имеющихся логов — backend stderr capture не было.

**Fix.** [commit hash] — `replace_all` теперь дедупит paths через `HashSet<&str>` перед `INSERT`. Дедуп выбран вместо `INSERT OR REPLACE` потому что:

1. Дубликаты в input — **сигнал** что scanner возвращает мусор (полезно ловить в логах позже, если такой dedup-counter добавим).
2. `INSERT OR REPLACE` молча перезаписывал бы и при реальных багах upsert-логики.
3. FTS-индекс при дубликатах путей дал бы две записи и поиск возвращал бы файл дважды.

**Регрешн-защита.** `services/kepler-backend/src/file_index/store.rs::tests::replace_all_dedupes_duplicate_paths` — кидает в `replace_all` три записи с двумя одинаковыми path'ами, проверяет что `Ok` и в БД ровно 2 строки.

**Prevention.**

- **Любой массовый bulk-insert по сырым данным из внешнего источника (FS scanner, network discovery, third-party API) обязан дедупиться перед `INSERT`.** SQLite UNIQUE — это контракт, который ты обещаешь соблюсти, не реактивный валидатор.
- **`std::sync::Mutex<Connection>` в долгих транзакциях, вызываемых из async task'а — анти-паттерн.** При 700k вставках держит worker thread tokio. Лучше: `tokio::task::spawn_blocking` или отдельный thread с channel'ом. См. [`db-resilience.md`](/concepts/db-resilience).
- **Backend должен писать stderr в файл, не только stdout.** Если stdout пропадает (zombie pipe), мы теряем все следы. Сейчас `crash_reporter` ловит panic, но stuck-без-panic — нет. Возможный TODO: периодический heartbeat-лог.
- **Supervisor должен делать health-check WS, а не только pid-alive.** Сейчас pid жив → supervisor спит, даже если WS-server stuck.

**Связанные правила.** [forbidden.md § Rust](/agents/forbidden) — про Mutex poison recovery, `RUST_BACKTRACE=1`, и обязательный `db_backup::maybe_backup_on_startup`. Этот случай показывает что недостаточно — нужен ещё health-check.
