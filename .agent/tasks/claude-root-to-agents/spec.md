# Task: replace .claude root with symlink to .agents

## Goal

Make `.agents` the single canonical directory by replacing the entire `.claude` directory with a symlink to `.agents`.

## Acceptance Criteria

- AC1: `/workspace/.claude` is a symlink after implementation.
- AC2: `/workspace/.claude` resolves to `/workspace/.agents`.
- AC3: Paths under `.claude` for shared assets continue to work via the symlink, specifically `.claude/skills` and `.claude/agents` exist and resolve through `.agents`.
- AC4: `.agents/skills` and `.agents/agents` remain present after the change.

## Verification Plan

- Record `ls -ld` and `readlink` output for `.claude`.
- Verify `.claude/skills` and `.claude/agents` exist.
- Verify `.agents/skills` and `.agents/agents` still exist.
