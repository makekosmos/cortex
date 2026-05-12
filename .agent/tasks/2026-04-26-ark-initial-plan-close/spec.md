# 2026-04-26 ARK initial plan close

## Scope

Close the remaining practical items from the original ARK platform plan that can
be completed safely now:

- Add a single automated ARK smoke runner that uses isolated task-local test
  databases only.
- Document the Eden Heart boundary correctly: Heart remains the Rust layer for
  heavy editor/vault/search work, while shared note/type/link data is represented
  as ARK objects.
- Document and guard the read-only SQL boundary: Dashboard may inspect ARK DBs
  read-only; Arrancador may keep read-only query paths until dedicated ARK query
  endpoints exist; app services must not write ARK tables directly.
- Refresh docs/TODO so the remaining work is actionable and current.

## Acceptance Criteria

AC1. Root package exposes a single ARK smoke command.

AC2. The smoke command creates/uses only paths under `.agent/tasks/<TASK_ID>/`
or another explicit isolated test location.

AC3. The smoke command covers ARK core, usage-tracker, `@kepler/ark`,
Arrancador tests/typecheck, Eden migration/build/typed-note e2e, and Dashboard
seed/analytics smoke.

AC4. Documentation explains the Eden Heart/ARK split without implying Heart
should be removed.

AC5. Documentation explains read-only ARK SQL boundaries and the difference
between allowed inspection reads and forbidden app-service writes.

AC6. A guard/check exists to detect direct ARK table writes in app TypeScript
services.

AC7. Fresh verification runs complete and results are recorded in `evidence.md`,
`evidence.json`, and raw artifacts.

## Out of Scope

- Removing Heart.
- Replacing every read-only ARK SQLite query with a new ARK endpoint in this
  pass.
- Manual testing against a main/user ARK database.
