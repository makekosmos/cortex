<!-- repo-task-proof-loop:start -->
## Repo task proof loop

For substantial features, refactors, and bug fixes, use the repo-task-proof-loop workflow.

Required artifact path:
- Keep all task artifacts in `.agent/tasks/<TASK_ID>/` inside this repository.

## Test database isolation

All tests, smoke checks, Playwright runs, and migration verification MUST use isolated test databases or temporary databases. Never point automated checks at a main/user ARK database. If a test needs ARK data, create it under `.agent/tasks/<TASK_ID>/`, an app-local `.tmp`/`.e2e` folder, or an OS temp directory, and pass the path explicitly through the app/test config.

Required sequence:
1. Freeze `.agent/tasks/<TASK_ID>/spec.md` before implementation.
2. Implement against explicit acceptance criteria (`AC1`, `AC2`, ...).
3. Create `evidence.md`, `evidence.json`, and raw artifacts.
4. Run a fresh verification pass against the current codebase and rerun checks.
5. If verification is not `PASS`, write `problems.md`, apply the smallest safe fix, and reverify.

Hard rules:
- Do not claim completion unless every acceptance criterion is `PASS`.
- Verifiers judge current code and current command results, not prior chat claims.
- Fixers should make the smallest defensible diff.

Installed workflow agents:
- `.agents/agents/task-spec-freezer.toml`
- `.agents/agents/task-builder.toml`
- `.agents/agents/task-verifier.toml`
- `.agents/agents/task-fixer.toml`
- `.agents/agents/task-spec-freezer.md`
- `.agents/agents/task-builder.md`
- `.agents/agents/task-verifier.md`
- `.agents/agents/task-fixer.md`
<!-- repo-task-proof-loop:end -->
