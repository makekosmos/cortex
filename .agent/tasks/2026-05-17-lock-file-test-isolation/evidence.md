# Evidence — 2026-05-17 lock-file-test-isolation

Все AC из `spec.md` верифицированы против текущего кода.

## AC1. Windows ветка `apply_owner_only_permissions` уважает `KOSMOS_LOCK_PERMISSIONS_DISABLED=1`

**PASS.** `services/kepler-backend/src/lock_file.rs` — в `#[cfg(windows)] fn apply_owner_only_permissions` добавлена ранняя проверка `lock_permissions_disabled()`:

```rust
if lock_permissions_disabled() {
    eprintln!(
        "[kepler-backend] {LOCK_PERMISSIONS_DISABLED_ENV}=1 — icacls hardening skipped \
         for {} (test-only path, prod должен не выставлять флаг)",
        path.display()
    );
    return Ok(());
}
```

Helper `lock_permissions_disabled()` читает env var `KOSMOS_LOCK_PERMISSIONS_DISABLED` и сравнивает с `"1"`.

## AC2. Unix ветка симметрична

**PASS.** В `#[cfg(unix)] fn apply_owner_only_permissions` добавлена та же ранняя проверка, после которой `chmod 0600` пропускается. Поведение детерминистично на обеих платформах.

## AC3. `launchKepler` выставляет флаг

**PASS.** `tests/e2e/helpers/launch.ts` теперь содержит:

```ts
KOSMOS_HEADLESS: "1",
// Test-only: backend пропускает icacls/chmod hardening на kepler.lock.json...
KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
KEPLER_SKIP_SYNC: "1",
```

## AC4. Rust unit test для skip-сценария

**PASS.** Добавлен `lock_file::tests::permissions_disabled_env_skips_hardening`:

- Сериализован через `ENV_MUTEX` (Mutex local to test mod) — `kosmos_data_dir_respects_env_override`, `unix_permissions_are_0600`, `windows_acl_inheritance_disabled` тоже теперь acquire его, чтобы исключить race.
- Тест ставит env var, вызывает `write_atomic`, снимает env var, верифицирует что файл создан и читаем, на Unix дополнительно проверяет что mode не 0600.

## AC5. `cargo test --manifest-path services\kepler-backend\Cargo.toml --lib` зелёный

**PASS.** См. `raw/cargo-test.md`. 58 passed, 0 failed.

## AC6. `bun run --cwd shell typecheck` зелёный

**PASS.** См. `raw/shell-typecheck-build.md`. `tsc --noEmit` exit 0.

## AC7. `bun run --cwd shell build:js` зелёный

**PASS.** См. `raw/shell-typecheck-build.md`. Все extensions собрались.

## AC8. `docs-site/agents/testing.md` обновлён

**PASS.** Раздел "Stale ACL lock-files (Windows)" теперь сначала описывает штатное решение (env-флаг + что `launchKepler` его выставляет), затем warning что prod не должен его выставлять, затем одноразовый cleanup для накопленных stale файлов. Блок "Открытый вопрос" удалён.

## AC9. `bun run docs:sync` + `bun run docs:check` зелёные

**PASS.** См. `raw/docs.md`. Регенерация прошла, stale references не найдено.
