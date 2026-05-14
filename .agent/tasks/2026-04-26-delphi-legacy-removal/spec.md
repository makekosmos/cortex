# Task: Remove Delphi legacy sidecar after ARK migration

## Context

Delphi now uses ARK task objects through `ark-core-rpc` / `@kosmos/ark`. The old `apps/delphi/ts/sidecar` Rust binary is no longer intended to be a runtime source of truth. The user confirmed this is a full dev migration and legacy should be removed rather than kept as a long-term fallback.

All verification must use isolated test databases/temp app-data only.

## Acceptance Criteria

- AC1: Delphi packaging and scripts no longer build, copy, or reference the legacy `delphi-db` sidecar.
- AC2: Runtime code does not depend on the legacy sidecar after ARK task-object migration.
- AC3: Obsolete legacy sidecar source is removed or clearly excluded from runtime with no package path using it.
- AC4: Eden shared note entry/type/search reads no longer use Heart as a permanent fallback after ARK object migration; Heart remains available for migration/export/folder/vault-specific work.
- AC5: Docs/TODO describe ARK `task_obj` as the Delphi source of truth and do not tell future agents to keep `delphi-db` as a fallback.
- AC6: Fresh verification passes on current code with test DB isolation.
