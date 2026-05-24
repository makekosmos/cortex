# 2026-05-24 — File Search scopes, ignores, NTFS mode

## Цель

Сделать Kepler File Search управляемым из Settings: пользователь выбирает папки поиска, настраивает ignore patterns, включает/выключает `.gitignore`, hidden files и ускоренный NTFS-режим. Дефолт должен работать без admin: `%USERPROFILE%` вместо полного обхода дисков.

## Контекст

Текущий индексатор уже умеет искать по локальному FTS index в `file-index.db`, но initial scan по fixed drives застревает в больших системных деревьях. Есть NTFS fast path через `kepler-focus-svc`/direct volume access, но он не должен быть единственным способом получить рабочий поиск. Настройки сейчас содержат только один toggle `exclude_noisy_folders`.

## Scope

- Backend persistence для search scopes, ignore patterns, `respect_gitignore`, `include_hidden`, `ntfs_accelerated`.
- Scanner options: user-mode scan с hardcoded ignores + user patterns + `.gitignore` support.
- Default scopes: `%USERPROFILE%`, если `KEPLER_FILE_INDEX_ROOTS` не задан и `KOSMOS_TEST_MODE != 1`.
- Settings UI: список scopes, add/remove folder picker, список ignore patterns, add/remove, toggles `.gitignore`, hidden files, NTFS accelerated mode, manual rescan.
- WS API под `file_index.*` для чтения и изменения этих настроек.
- Regression tests для persistence/scanner/settings behavior.
- Обновить `MAKING-FILE-SEARCH-PLAN.md` и `docs-site/agents/postmortems.md`.

## Out of Scope

- Новый Windows service.
- Новый pipe protocol.
- Content/full-text file search.
- Network drives или mounted-volume UX.
- Installer/release bump.

## Acceptance Criteria

**AC1.** `file_index.settings_get` возвращает persisted settings: `roots`, `ignore_patterns`, `respect_gitignore`, `include_hidden`, `ntfs_accelerated`; fresh DB получает `%USERPROFILE%` default root outside test mode.

**AC2.** Settings UI «Поиск файлов» позволяет добавить и удалить search scope через native folder picker; удаление scope удаляет indexed files из этого subtree или следующий rescan не возвращает их.

**AC3.** Settings UI позволяет добавить и удалить ignore pattern вроде `*.tmp`; invalid/blank duplicates не ломают Settings, не создают дубликаты и показывают понятную ошибку.

**AC4.** User-mode scan игнорирует hardcoded noisy paths (`node_modules`, `.git`, `target`, `dist`, `build`, `AppData`, cache/temp) и пользовательские patterns.

**AC5.** Toggle «Учитывать .gitignore» включает/выключает respect `.gitignore` при user-mode scan.

**AC6.** Toggle «Показывать скрытые файлы» по умолчанию OFF; hidden files не индексируются при OFF и индексируются при ON.

**AC7.** Toggle «Ускоренный NTFS-режим» управляет использованием existing NTFS fast path. OFF гарантированно использует user-mode scanner; ON пробует existing NTFS path для drive roots и gracefully falls back to user-mode.

**AC8.** Watcher/rescan не использует stale roots после settings change. Если dynamic watcher restart окажется слишком рискованным, watcher можно явно отключить/restart on change, но settings/result consistency после manual rescan должна быть корректной.

**AC9.** `cargo test -p kepler-backend file_index::` PASS.

**AC10.** `bun run --cwd shell typecheck` PASS.

**AC11.** `bun run ark:guard:writes` PASS.

**AC12.** `docs-site/agents/postmortems.md` entry про file search обновлён: статус исправлен, есть fix, regression protection и prevention.

## Notes

- `exclude_noisy_folders` остаётся backward-compatible, но UI может перейти на новые explicit settings.
- Existing `scanner/ntfs.rs` не удаляется в этой задаче; новый toggle только делает его honest opt-in/opt-out.
