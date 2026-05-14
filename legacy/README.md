# Legacy code

Замороженный код. Workspace member'ом не является — оставлен для возможного отката Phase E refactor'а.

## usage-tracker (standalone Rust binary)

Заморожен 2026-05-14 после Phase E refactor'а — логика win32 active-window capture переехала в `services/kepler-backend/src/usage_tracker/` как in-process tokio task внутри backend'а. Standalone exe больше не запускается через HKCU\Run autostart; backend стартует через Kepler shell spawn и сам поддерживает tracking.

Реактивация (выкатка standalone exe обратно) требует **proof loop** — отдельная задача в `.agent/tasks/`.

### Чтобы собрать замороженную копию вручную (для rollback / диагностики)

```powershell
cargo build --release --manifest-path legacy/usage-tracker/Cargo.toml
cargo run --release --manifest-path legacy/usage-tracker/Cargo.toml -- --once
```

Standalone exe пишет в ARK напрямую через `ark_core::db::*` (либо через WS к kepler-backend если `USAGE_TRACKER_USE_KEPLER=1`). Не должен запускаться одновременно с backend, у которого `KEPLER_USAGE_TRACKER` НЕ выставлен в `0` — будут дубликаты sessions.

### Installer (HKCU autostart)

Сценарии `installer/install.ps1` и `installer/uninstall.ps1` остаются для исторического справочного материала. Они **не** должны выполняться на машине, где запущен новый kepler-backend — backend уже автостартит и собственно делает tracking. Если нужно поставить замороженную standalone-версию параллельно для диагностики — сначала временно установи `KEPLER_USAGE_TRACKER=0` для backend'а, чтобы не было дубликатов.
