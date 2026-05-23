# 2026-05-23 — SingletonGuard: cross-process тест ungrateful termination

## Контекст

В рамках `.agent/tasks/2026-05-23-singleton-pid-reuse/` убран pid-based гейт,
весь singleton-инвариант теперь держится на `SingletonGuard` (SQLite WAL
`BEGIN IMMEDIATE` на `kepler-singleton.lock.db`). Это OS-level file lock —
kernel освобождает handle при любой смерти процесса, что и обеспечивает
«больше никогда» обещание после фикса pid reuse.

Существующие тесты в `services/kepler-backend/src/singleton.rs`:

- `second_acquire_fails_fast` — два `acquire` подряд внутри одного процесса.
- `re_acquire_after_drop_works` — happy-path Rust `Drop`.
- `second_acquire_clearing_stale_lock_fails_with_already_running` — то же
  in-process, плюс контракт error message.

**Чего НЕТ.** Cross-process валидация. In-process конфликт двух
`BEGIN IMMEDIATE` идёт через SQLite connection-state, не через `fcntl` /
`LockFileEx`. Поэтому текущие тесты НЕ доказывают:

1. Что second `acquire` из второго процесса упадёт пока первый держит lock.
2. Что после `TerminateProcess` / SIGKILL первого процесса (без graceful
   Drop) lock освобождается kernel'ом и второй `acquire` проходит.

После pid-reuse фикса SingletonGuard — единственная точка отказа для
singleton-инварианта. Полагаться на «должно работать по теории» без теста
— это handwave, на который наступим если SQLite/Cargo/Windows поведение
поменяется (например, оверлапованные I/O на Win не отдают handle сразу —
есть упоминания на dev mailing list).

## Решение

Test-only binary + integration test, который spawn'ит этот binary как child,
проверяет live-conflict и post-kill release. Не используем
`CARGO_BIN_EXE_kepler-backend` — полный backend на старте поднимает tracing,
ARK, WS, file index; всё irrelevant к singleton'у, делает тест flaky и
медленным.

### Test-only binary

Файл `services/kepler-backend/tests/bin/singleton-holder.rs`. Делает ровно:

```rust
fn main() {
    let lock_path = std::env::args().nth(1).expect("lock_path");
    let singleton_path = std::env::args().nth(2).expect("singleton_path");
    let (_guard, _stale) = kepler_backend::singleton::acquire_clearing_stale_lock(
        std::path::Path::new(&lock_path),
        std::path::Path::new(&singleton_path),
    ).expect("acquire");
    println!("acquired");
    use std::io::Write;
    std::io::stdout().flush().unwrap();
    // Hold lock пока parent не убьёт или не закроет stdin.
    let mut buf = String::new();
    let _ = std::io::stdin().read_line(&mut buf);
}
```

**В Cargo.toml явно объявить** (auto-discovery `tests/bin/*` Cargo не делает
на stable — только `src/bin/*`):

```toml
[[bin]]
name = "singleton-holder"
path = "tests/bin/singleton-holder.rs"
test = false
doc = false
```

`env!("CARGO_BIN_EXE_singleton-holder")` будет доступен в integration tests.

### Integration test

Файл `services/kepler-backend/tests/singleton_cross_process.rs`. Два теста:

**Test 1 — live conflict.** Spawn singleton-holder, дождаться `acquired` строки
из child.stdout (sync через stdout, НЕ `thread::sleep`). Из родительского
процесса вызвать `acquire_clearing_stale_lock` на тот же singleton_path
→ assert `Err(SingletonError::AlreadyRunning)`. Затем закрыть child stdin —
child выходит, lock освобождается. Доп. retry на parent acquire с
exponential backoff (макс ~500ms) для подтверждения graceful release.

**Test 2 — ungrateful termination.** Spawn singleton-holder, дождаться
`acquired`. Вызвать `child.kill()` (на Windows `TerminateProcess`,
кросс-платформенно через `std::process::Child::kill`). `child.wait()`.
Затем parent пробует `acquire_clearing_stale_lock` — должен пройти.
**Retry с backoff** (3 попытки: 50ms / 100ms / 200ms) — Windows может
держать handle десятки миллисекунд если есть оверлапованные I/O, мы хотим
протестировать «handle освобождается eventually», не «синхронно с exit».
Если за 500ms не освободился — assert fail (реальная регрессия).

## Файлы

- `services/kepler-backend/Cargo.toml` — `[[bin]]` block для
  singleton-holder + `test = false`/`doc = false`.
- `services/kepler-backend/tests/bin/singleton-holder.rs` — новый minimal
  bin.
- `services/kepler-backend/tests/singleton_cross_process.rs` — новый
  integration test файл с двумя тестами.

## Acceptance Criteria

- **AC1.** `singleton-holder` минимальный: импортирует только
  `kepler_backend::singleton`, никаких других модулей. Никакого
  tracing init, никакого tokio runtime. Holder выходит из main когда stdin
  закрывается ИЛИ когда parent kill'ает.
- **AC2.** Live-conflict тест: while child alive + holding lock, parent
  `acquire_clearing_stale_lock` возвращает ровно
  `Err(SingletonError::AlreadyRunning)` (не другие варианты `SingletonError`).
- **AC3.** Graceful release: после закрытия child stdin parent acquire
  проходит в пределах 500ms. Retry с backoff, не fixed sleep.
- **AC4.** Ungrateful termination: после `child.kill()` + `child.wait()`
  parent acquire проходит в пределах 500ms. Это **ключевой тест** «больше
  никогда» — он валидирует kernel-level handle release.
- **AC5.** Negative path в обоих тестах: до child kill / stdin close
  acquire **должен падать** с `AlreadyRunning`. Без этой проверки positive
  path ничего не доказывает (вдруг acquire всегда успешен).
- **AC6.** Sync между parent и child — через child.stdout
  (`BufRead::read_line` пока не прочитает `"acquired"`), НЕ через
  `thread::sleep`. Иначе тест flaky на медленных CI agents.
- **AC7.** Тест НЕ помечен `#[ignore]`. Если медленный (>2s) — оформить
  как отдельный test target в `[[test]] harness = false` или через
  `#[cfg(feature = "slow-tests")]`, но в дефолтном `cargo test` запускается.
- **AC8.** Кросс-платформенно работает (Win + Unix) — `Child::kill` в std
  кросс-платформенный, прочие APIs тоже. Если на одной из платформ
  поведение различается (например, timing) — описать в комментарии теста.
- **AC9.** `cargo test --manifest-path services\kepler-backend\Cargo.toml`
  зелёный (включая новые тесты).
- **AC10.** Обновить комментарий в
  `singleton.rs::tests::second_acquire_clearing_stale_lock_fails_with_already_running`
  — убрать ссылку на «cross-process валидация запланирована», заменить на
  «cross-process покрыто `tests/singleton_cross_process.rs`».

## Запреты

- Не использовать `CARGO_BIN_EXE_kepler-backend` (полный backend = слишком
  много стороннего state, flaky тест).
- Не использовать fixed `thread::sleep` для синхронизации между parent
  и child (только sync через stdout / retry с backoff на acquire).
- Не помечать тесты `#[ignore]` под предлогом «slow» — отдельный target если
  надо, но НЕ ignore (cross-process тесты — частая жертва «временно
  ignored», которая остаётся ignored навсегда).
- Не трогать существующие in-process тесты (`second_acquire_fails_fast`,
  `re_acquire_after_drop_works`,
  `second_acquire_clearing_stale_lock_fails_with_already_running`) — они
  тестят другой слой (Rust API контракт), оставить как complementary
  coverage.
- Не менять public API `SingletonGuard` / `acquire_clearing_stale_lock` —
  тесты должны использовать ровно тот же entry point, что и `main.rs`.

## Что вне scope

- Тест на panic во время `acquire_clearing_stale_lock` — purely Rust-level
  drop semantics, уже косвенно покрыто `re_acquire_after_drop_works`
  (panic = unwind = Drop).
- Тест на SMB / network drive — out-of-scope (см.
  `docs-site/concepts/db-resilience.md` § «Что НЕ покрывают»).
- Retry с backoff на самом `acquire_clearing_stale_lock` против AV-задержки
  на `fs::remove_file` — пока не наблюдалось, добавим только если в проде
  начнут лететь `SingletonError::ClearStaleLock`.

## Связанное

- Парент-задача: `.agent/tasks/2026-05-23-singleton-pid-reuse/spec.md` (фикс
  pid reuse, который оставил SingletonGuard единственной точкой отказа).
- Postmortem: `docs-site/agents/postmortems.md` § 2026-05-23 — Kepler:
  singleton conflict из-за pid reuse.
- Гэп в текущих тестах задокументирован in-code в `singleton.rs::tests::
second_acquire_clearing_stale_lock_fails_with_already_running`.
