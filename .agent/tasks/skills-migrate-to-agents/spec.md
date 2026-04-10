# Task: migrate Claude skills/agents into .agents and link .claude to .agents

## Goal
Make `.agents` the canonical home for shared agent assets by moving/copying skills and agents from `.claude` into `.agents`, then replacing `.claude/skills` and `.claude/agents` with symlinks to `.agents/skills` and `.agents/agents`.

## Acceptance Criteria
- AC1: Every directory that exists directly under `/workspace/.claude/skills` before implementation also exists directly under `/workspace/.agents/skills` after implementation.
- AC2: Every file that exists directly under `/workspace/.claude/agents` before implementation also exists under `/workspace/.agents/agents` after implementation.
- AC3: `/workspace/.claude/skills` is a symlink to `/workspace/.agents/skills` (or equivalent relative symlink target).
- AC4: `/workspace/.claude/agents` is a symlink to `/workspace/.agents/agents` (or equivalent relative symlink target).
- AC5: Existing `.agents/skills` content remains present after migration.

## Verification Plan
- Capture pre/post directory listings.
- Verify symlink targets with `readlink`/`ls -ld`.
- Verify expected names in `.agents/skills` and `.agents/agents`.
