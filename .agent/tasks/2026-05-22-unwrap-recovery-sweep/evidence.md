# Evidence — unwrap-recovery-sweep

## AC1 [unwrap-prod-zero]: PASS

```
> cargo clippy --workspace --lib --bins 2>&1 | grep clippy::unwrap_used
(no output)
```

До правки: 27 `unwrap_used` warning'ов в prod paths
(`crates/ark-core/rust/src/{ffi.rs,main.rs,mesh.rs,relay_transport.rs}`,
`services/ark-relay-server/src/relay.rs`,
`services/kepler-backend/src/ws_server.rs`).

После правки: **0**.

Распределение фиксов:

- `crates/ark-core/rust/src/ffi.rs` — 8 fixes
  (Mutex poison recovery `lock().unwrap_or_else(|e| e.into_inner())`)
- `crates/ark-core/rust/src/main.rs` — 5 fixes (Mutex poison recovery)
- `crates/ark-core/rust/src/mesh.rs` — 1 fix (Mutex poison recovery)
- `crates/ark-core/rust/src/relay_transport.rs` — 7 fixes (Mutex)
- `services/ark-relay-server/src/relay.rs` — 5 fixes (Mutex)
- `services/kepler-backend/src/ws_server.rs` — 1 fix
  (`"127.0.0.1:0".parse().unwrap()` → `.expect("hardcoded socket literal
is always valid")`)

**Total: 27 prod unwrap'ов починено.**

## AC2 [unwrap-deny-active]: PASS

`Cargo.toml` (корень):

```toml
[workspace.lints.clippy]
unwrap_used = "deny"
panic = "warn"
todo = "warn"
unimplemented = "warn"
```

## AC3 [tests-allow-attribute]: PASS

`#![cfg_attr(test, allow(clippy::unwrap_used))]` добавлен в crate root'ы:

- `crates/ark-core/rust/src/lib.rs`
- `crates/ark-core/rust/src/main.rs`
- `services/kepler-backend/src/lib.rs`
- `services/kepler-backend/src/main.rs`
- `services/ark-relay-server/src/main.rs`
- `services/kepler-focus-svc/src/lib.rs`
- `services/kepler-focus-svc/src/main.rs`
- `services/kepler-focus-helper/src/lib.rs`

`#![allow(clippy::unwrap_used)]` (file-level) в integration tests и benches:

- `crates/ark-core/rust/tests/relay_round_trip.rs`
- `crates/ark-core/rust/tests/sync_round_trip.rs`
- `crates/ark-core/rust/tests/proptest_invariants.rs`
- `services/kepler-backend/tests/arrancador_scanner_test.rs`
- `crates/ark-core/rust/benches/delphi_filters.rs`
- `crates/ark-core/rust/benches/pomodoro_session.rs`

Итого: 8 crate root + 6 integration tests/benches = **14 файлов** получили
allow attribute.

## AC4 [clippy-all-targets-no-errors]: PASS

```
> cargo clippy --workspace --all-targets 2>&1 | grep '^error'
(no output)
```

Остаются warning'и других lint'ов (panic, sort_by_key, manual_contains,
io_other_error, derivable_impls, type_complexity, result_large_err,
too_many_arguments, dead_code) — все вне scope этой задачи.

## AC5 [tests-pass]: PASS

```
> cargo test --workspace --lib 2>&1 | grep '^test result'
test result: ok. 162 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.27s
test result: ok. 147 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.85s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Всего **332 unit теста** passed, 0 failed.

`cargo build --workspace` не прогнан до конца — в момент verify пользователь
держал запущенный Kepler (kepler-backend.exe / ark-core-rpc.exe залочены),
`cargo build` не смог пересобрать bin targets. Clippy и `cargo test --lib
--no-run` успешно type-check'нули весь workspace (включая bin targets через
clippy --bins). Build будет green после restart Kepler'а.

## AC6 [mutex-recovery-canonical]: PASS

Все remaining `lock().unwrap()` под `crates/` и `services/` живут внутри
`#[cfg(test)] mod tests` блоков (verified ручной проверкой —
`#[cfg(test)]` маркеры найдены выше каждого hit'а):

- `crates/ark-core/rust/src/main.rs:1483,1615,1781` — test mod (тесты sync роутера)
- `crates/ark-core/rust/src/relay_sync.rs:432,453,471` — test mod (mock relay server)
- `crates/ark-core/rust/src/sync_server.rs:1031,1035,1045` — test mod
- `services/kepler-backend/src/focus.rs:710-1243` — test mod (FakeArk mock)
- `services/kepler-backend/src/crash_reporter.rs:136,142` — test mod
- `services/kepler-backend/src/arrancador/rawg.rs:354-560` — test mod (FakeArk mock)

Prod paths используют canonical `unwrap_or_else(|e| e.into_inner())` для
Mutex poison recovery (forbidden.md → Mutex discipline).

## Summary

- 27 prod unwrap'ов починено (canonical recovery pattern)
- 14 файлов получили test-scoped allow attribute
- `unwrap_used` поднят с warn до deny на workspace level
- 332 unit теста проходят
- 0 clippy errors после поднятия до deny

Дальше CI блокирует регрессию: любой новый `unwrap()` в prod paths =
compile error.
