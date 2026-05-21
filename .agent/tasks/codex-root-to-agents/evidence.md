# Evidence: replace .codex root with symlink to .agents

## Summary

Copied Codex agent definitions from `.codex/agents` into `.agents/agents`, then replaced the entire `/workspace/.codex` directory with a symlink to `/workspace/.agents`.

## Acceptance Criteria Results

### AC1

Every file that existed directly under `/workspace/.codex/agents` before implementation also exists under `/workspace/.agents/agents` after implementation.

Result: PASS

Evidence:

- Post-migration `.agents/agents` contains:
  - `task-builder.toml`
  - `task-fixer.toml`
  - `task-spec-freezer.toml`
  - `task-verifier.toml`
- Raw artifacts:
  - `.agent/tasks/codex-root-to-agents/raw/pre-codex-agent-files.txt`
  - `.agent/tasks/codex-root-to-agents/raw/post-agents-agent-files.txt`

### AC2

`/workspace/.codex` is a symlink after implementation.

Result: PASS

Evidence:

- `ls -ld .codex` => `lrwxrwxrwx ... .codex -> .agents`
- Raw artifact: `.agent/tasks/codex-root-to-agents/raw/post-codex-ls.txt`

### AC3

`/workspace/.codex` resolves to `/workspace/.agents`.

Result: PASS

Evidence:

- `readlink .codex` => `.agents`
- Verification confirms resolved path equals `/workspace/.agents`
- Raw artifacts:
  - `.agent/tasks/codex-root-to-agents/raw/readlink-codex.txt`
  - `.agent/tasks/codex-root-to-agents/raw/verification.txt`

### AC4

`/workspace/.codex/agents` exists through the symlink and contains the expected Codex agent files.

Result: PASS

Evidence:

- Verification confirms `.codex/agents` contains the original Codex `.toml` files
- Raw artifacts:
  - `.agent/tasks/codex-root-to-agents/raw/post-codex-agent-files.txt`
  - `.agent/tasks/codex-root-to-agents/raw/verification.txt`

### AC5

Existing `.agents/agents` content remains present after the change.

Result: PASS

Evidence:

- Existing markdown agent files remain present alongside copied `.toml` files:
  - `task-builder.md`
  - `task-fixer.md`
  - `task-spec-freezer.md`
  - `task-verifier.md`
- Raw artifacts:
  - `.agent/tasks/codex-root-to-agents/raw/pre-agents-agent-files.txt`
  - `.agent/tasks/codex-root-to-agents/raw/post-agents-agent-files.txt`
