# Evidence

## Commands

- `rg completedCount|totalCount|progress|createProject|CreateProjectParams products/delphi/src products/delphi/tests`
- `bun run --cwd platform/desktop typecheck`
- `bun run --cwd platform/desktop build:extension delphi`
- `bunx desloppify scan --json . > .agent/tasks/2026-06-24-desloppify-delphi-project-model-dead-exports/desloppify-after.json`

## Results

- References check: PASS. Removed helper names had no cross-file references;
  `createProject` and `CreateProjectParams` remain used by `store/todos.ts`.
- `typecheck`: PASS.
- `build:extension delphi`: PASS.
- Baseline scan: score 0, 507 findings; severity critical 0, high 306,
  medium 130, low 71.
- Final scan: score 0, 504 findings; severity critical 0, high 303,
  medium 130, low 71.
- `products/delphi/src/models/project.ts` `DEAD_EXPORT`: 3 -> 0.

## AC Verdicts

- AC1: PASS. Baseline and final JSON are saved.
- AC2: PASS. Confirmed dead project helpers are removed.
- AC3: PASS. `createProject` and `CreateProjectParams` remain exported and
  typechecked.
- AC4: PASS. Relevant TypeScript checks passed.

## Deferred

- Repo-wide score remains 0 because many high `DEAD_FILE`/`DEAD_EXPORT`
  findings remain outside this slice.
