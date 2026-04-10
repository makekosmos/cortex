# Task: replace .codex root with symlink to .agents

## Goal
Make `.agents` the single canonical directory for Codex-facing assets too by copying `.codex/agents` contents into `.agents/agents` and then replacing the entire `.codex` directory with a symlink to `.agents`.

## Acceptance Criteria
- AC1: Every file that exists directly under `/workspace/.codex/agents` before implementation also exists under `/workspace/.agents/agents` after implementation.
- AC2: `/workspace/.codex` is a symlink after implementation.
- AC3: `/workspace/.codex` resolves to `/workspace/.agents`.
- AC4: `/workspace/.codex/agents` exists through the symlink and contains the expected Codex agent files.
- AC5: Existing `.agents/agents` content remains present after the change.

## Verification Plan
- Capture pre/post agent file listings.
- Verify `.codex` symlink with `ls -ld` and `readlink`.
- Verify `.codex/agents` and `.agents/agents` contents after migration.
