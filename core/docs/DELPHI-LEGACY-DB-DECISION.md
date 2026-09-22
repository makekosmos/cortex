# Delphi Legacy DB Removal

Decision: the old Delphi-specific DB sidecar is removed.

Current replacement for shared task state is ARK object storage:

- task data: `objects` with `type_id = task_obj`;
- typed/schema metadata: `object_types`;
- relationships: `object_links`;
- app access: `@kosmos/ark` / `ark-core-rpc`.

Rules:

- Do not package or restore `apps/delphi/ts/sidecar`.
- Do not use old todo tables as a long-term fallback after task-object
  migration.
- Delphi startup may read legacy todos only to migrate them into `task_obj`.
- After migration, `task_obj` objects are the source of truth.
