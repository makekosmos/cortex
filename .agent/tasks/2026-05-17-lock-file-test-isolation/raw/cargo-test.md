# cargo test --manifest-path services\kepler-backend\Cargo.toml --lib

Result: **PASS**

```
test result: ok. 58 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.21s
```

Relevant tests:

- `lock_file::tests::permissions_disabled_env_skips_hardening` — NEW, ok
- `lock_file::tests::windows_acl_inheritance_disabled` — ok (флаг сброшен под ENV_MUTEX, ACL применился)
- `lock_file::tests::kosmos_data_dir_respects_env_override` — ok (теперь под ENV_MUTEX)
- `lock_file::tests::write_and_read_roundtrip` — ok
- `lock_file::tests::write_overwrites_existing` — ok
- `lock_file::tests::write_creates_parent_directory` — ok
- `lock_file::tests::read_if_alive_*` — ok (все 3)
- `lock_file::tests::read_nonexistent_returns_notfound` — ok
