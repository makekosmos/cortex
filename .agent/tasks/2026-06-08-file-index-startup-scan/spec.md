# 2026-06-08 — File index startup scan safety

## Цель

Убрать опасный startup-путь, где file index без явного opt-in сканирует весь `%USERPROFILE%`, и вынести тяжелый filesystem/SQLite rescan с async runtime workers.

## Скоуп

- Feature flags для file index и initial rescan.
- Безопасные default roots: без `KEPLER_FILE_INDEX_ROOTS` индекс не должен сам брать `%USERPROFILE%`.
- Blocking filesystem walk и `replace_all` не должны выполняться на tokio worker thread.
- Running scan должен кооперативно отменяться при изменении настроек/root'ов.
- Не меняем пользовательский UI настроек file search.
- Не реализуем полноценный incremental diff index вместо `replace_all`; для этой задачи достаточно отмены stale scan до commit и blocking isolation.

## Acceptance Criteria

**AC1.** Cold start с `KEPLER_FILE_INDEX_INITIAL_RESCAN=0` не запускает initial file index rescan и не делает full profile scan.

**AC2.** Без `KEPLER_FILE_INDEX_ROOTS` и вне test mode `scanner::default_roots()` возвращает пустой список, а `%USERPROFILE%` не seed'ится в `file_index_roots`.

**AC3.** `FileIndex::rescan()` выполняет `scan_roots_with_progress` и `store.replace_all` через `tokio::task::spawn_blocking`, чтобы long filesystem/SQLite job не занимал tokio worker.

**AC4.** Изменение settings/root'ов во время scan отменяет старый scan кооперативно до `replace_all`; regression test подтверждает, что cancel token останавливает scanner loop быстрее 1 секунды на synthetic large tree.

**AC5.** `KEPLER_FILE_INDEX=0` отключает file index на уровне startup behavior: backend не запускает initial rescan, file search/settings API остаются безопасными и возвращают пустой index без background scan.

**AC6.** Релевантные проверки проходят: targeted Rust tests для file index, `cargo test -p kepler-backend file_index`, `bun run docs:check`, `bun run ark:smoke` или зафиксированная причина, если полный smoke недоступен локально.
