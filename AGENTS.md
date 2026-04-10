<!-- repo-task-proof-loop:start -->
## Repo task proof loop

For substantial features, refactors, and bug fixes, use the repo-task-proof-loop workflow.

Required artifact path:
- Keep all task artifacts in `.agent/tasks/<TASK_ID>/` inside this repository.

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
- `.agents/agents/task-spec-freezer.md`
- `.agents/agents/task-builder.md`
- `.agents/agents/task-verifier.md`
- `.agents/agents/task-fixer.md`
- `.agents/agents/task-spec-freezer.toml`
- `.agents/agents/task-builder.toml`
- `.agents/agents/task-verifier.toml`
- `.agents/agents/task-fixer.toml`

## AGENTS.md usage

- `AGENTS.md` is the canonical agent-instructions filename in this repository.
- Agents should use the nearest relevant `AGENTS.md` for the part of the repo they are working in, in addition to this root file.
- If a subproject has its own `AGENTS.md` (for example under `apps/` or `packages/`), treat it as project-specific guidance for that subtree so work can proceed more effectively.
- Prefer updating and maintaining `AGENTS.md` files rather than reintroducing `CLAUDE.md` files.

Agent session note:
- If agent config files were just created or refreshed during a running session, start a new agent session before relying on the updated agent list.
- Use the agent listing command/tool of your environment to inspect available agents.
- Keep this block in the root `AGENTS.md`. If the workflow needs longer repo guidance, prefer path imports or dedicated project docs instead of expanding this block indefinitely.
<!-- repo-task-proof-loop:end -->
