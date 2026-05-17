# 2026-05-17 — lock-file test isolation env flag

## Контекст

`services/kepler-backend/src/lock_file.rs::apply_owner_only_permissions()`
при записи `kepler.lock.json` применяет жёсткий Windows ACL:

```rust
icacls <path> /inheritance:r /grant:r <USERNAME>:F
```

Это правильно для prod (lock содержит auth token к ARK DB). Но если username
на машине когда-то менялся (или другой Windows-аккаунт оставил файл), старые
lock'и становятся unreadable, текущий user не может удалить → `freshDataDir`
в e2e падает с `EPERM`.

## Решение

Новый env-флаг `KOSMOS_LOCK_PERMISSIONS_DISABLED=1`. Когда установлен —
`apply_owner_only_permissions` no-op'ит (с warning log). `launchKepler`
e2e helper всегда его выставляет.

Симметрично применяется и для Unix (chmod 0600 пропускается), чтобы поведение
было детерминированным и предсказуемым на всех платформах.

## Файлы

- `services/kepler-backend/src/lock_file.rs` — поправить `apply_owner_only_permissions` (обе ветки), добавить unit test.
- `tests/e2e/helpers/launch.ts` — добавить `KOSMOS_LOCK_PERMISSIONS_DISABLED: "1"`.
- `docs-site/agents/testing.md` — обновить раздел "Stale ACL lock-files", убрать "Открытый вопрос".

## Acceptance Criteria

- **AC1.** `apply_owner_only_permissions()` на Windows: если `KOSMOS_LOCK_PERMISSIONS_DISABLED == "1"` — `tracing::warn!` и `return Ok(())`, без `icacls`.
- **AC2.** `apply_owner_only_permissions()` на Unix: симметрично — пропускает chmod 0600 при том же флаге.
- **AC3.** `tests/e2e/helpers/launch.ts` выставляет `KOSMOS_LOCK_PERMISSIONS_DISABLED: "1"` рядом с `KOSMOS_HEADLESS: "1"`.
- **AC4.** Rust unit test в `lock_file.rs::tests`: при выставленном флаге запись проходит, файл существует и читаемый; ACL/permission hardening не применяется. Сериализация env-mutations через `Mutex` (или такого же паттерна, как в существующем `kosmos_data_dir_respects_env_override`).
- **AC5.** `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib` зелёный.
- **AC6.** `bun run --cwd shell typecheck` зелёный.
- **AC7.** `bun run --cwd shell build:js` зелёный.
- **AC8.** `docs-site/agents/testing.md` — раздел "Stale ACL lock-files" дополнен про флаг + что `launchKepler` его автоматически выставляет; блок "Открытый вопрос" удалён.
- **AC9.** `bun run docs:sync` + `bun run docs:check` зелёные.

## Запреты

- Не трогать `packages/visuals/*`, `extensions/*`, другие e2e спеки.
- Не менять prod-поведение (без env флага — точно тот же icacls/chmod).
- Не выставлять флаг где-то ещё кроме `launchKepler` helper и юнит-теста.
