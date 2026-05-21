# 2026-05-22 unwrap-recovery-sweep

## Context

Финал bug-detection roadmap. После Phase 1-7 (Mutex discipline, panic
recovery, clippy lints на warn, ts-rs coverage, boot self-check)
остаётся последний шаг: поднять `[workspace.lints.clippy].unwrap_used`
с `warn` до `deny`, чтобы CI блокировал regression в prod paths.

`forbidden.md → Mutex discipline` явно запрещает `Mutex::lock().unwrap()`
в production — один panic в одном thread'е poison'ит Mutex навсегда,
sync становится неработоспособен до process restart. Canonical recovery
pattern: `lock().unwrap_or_else(|e| e.into_inner())` (SQLite транзакции
atomic, данные внутри guard'а валидны).

В prod paths (lib + bin targets) накопилось 27 `unwrap_used` warning'ов
с момента включения clippy lint на warn. Тестовый код легитимно
использует `unwrap()` — там действует test-scoped allow attribute.

## Scope

В задаче:

- Найти все `unwrap()` warning'и в prod paths через
  `cargo clippy --workspace --lib --bins`.
- Заменить `Mutex/RwLock::lock().unwrap()` на
  `lock().unwrap_or_else(|e| e.into_inner())` (poison recovery).
- `Option::unwrap()` / `Result::unwrap()` на других типах — заменить
  на `expect("invariant message")` где invariant доказуем (hardcoded
  literal'ы, post-`is_some()` checks).
- Добавить `#![cfg_attr(test, allow(clippy::unwrap_used))]` на crate
  root каждого `lib.rs` / `main.rs` (для inline `#[cfg(test)] mod tests`).
- Добавить `#![allow(clippy::unwrap_used)]` на каждый файл под
  `tests/` и `benches/` (integration tests = отдельные crate'ы).
- Поднять `[workspace.lints.clippy].unwrap_used` с `"warn"` до `"deny"`.

Не в задаче:

- `panic!` / `todo!` / `unimplemented!` warnings — отдельный fix-проход.
- Refactor больших unwrap chain'ов в `?`-based error propagation.
- Замена `unwrap()` после `is_some()` на `if let Some(x) = ...` —
  refactor scope, выходит за «sweep» формат.

## Acceptance Criteria

- **AC1** [unwrap-prod-zero]: `cargo clippy --workspace --lib --bins`
  выдаёт **0** `clippy::unwrap_used` warning'ов.
- **AC2** [unwrap-deny-active]: `Cargo.toml` корня содержит
  `unwrap_used = "deny"` в `[workspace.lints.clippy]`.
- **AC3** [tests-allow-attribute]: Каждый файл, ранее эмитировавший
  unwrap warning'и в test-target build'е, получил
  `#![cfg_attr(test, allow(clippy::unwrap_used))]` (для inline
  `#[cfg(test)] mod tests` в lib/bin) или `#![allow(clippy::unwrap_used)]`
  (для `tests/*.rs` / `benches/*.rs`).
- **AC4** [clippy-all-targets-no-errors]: `cargo clippy --workspace
  --all-targets` завершается без `error[...]` (warning'и не-unwrap
  lint'ов допустимы — они вне scope).
- **AC5** [tests-pass]: `cargo test --workspace --lib` — все lib unit
  тесты passed (0 failed).
- **AC6** [mutex-recovery-canonical]: Все production `Mutex::lock()` /
  `RwLock::write()` / `RwLock::read()` используют
  `unwrap_or_else(|e| e.into_inner())`, а не голый `unwrap()` /
  `expect()`. Verify: `grep -rn 'lock().unwrap()'` под `crates/`
  и `services/` (за вычетом `#[cfg(test)]` блоков) → 0 hits.
