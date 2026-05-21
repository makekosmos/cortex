# Evidence: migrate Claude skills/agents into .agents and link .claude to .agents

## Summary

Migrated/copy-synced skills from `.claude/skills` into `.agents/skills`, copied agent files from `.claude/agents` into `.agents/agents`, then replaced `.claude/skills` and `.claude/agents` with symlinks to `.agents`.

## Acceptance Criteria Results

### AC1

Every directory that existed directly under `/workspace/.claude/skills` before implementation also exists directly under `/workspace/.agents/skills` after implementation.

Result: PASS

Evidence:

- Post-migration `.agents/skills` contains:
  - `android-120fps`
  - `ark-pair`
  - `ark-start`
  - `ark-sync`
  - `ark-test-env`
  - `compose-ime-insets`
  - `delphi-dev`
  - `dev-all`
  - `kotlin-android`
  - `offline-first`
  - `repo-task-proof-loop`
  - `swiftui-expert-skill`
  - `typecheck`
  - `update-docs`
  - `using-superpowers`
  - `vercel-react-native-skills`
  - `vue-best-practices`
- Raw artifact: `.agent/tasks/skills-migrate-to-agents/raw/post-agents-skill-dirs.txt`

### AC2

Every file that existed directly under `/workspace/.claude/agents` before implementation also exists under `/workspace/.agents/agents` after implementation.

Result: PASS

Evidence:

- `.agents/agents` contains:
  - `task-builder.md`
  - `task-fixer.md`
  - `task-spec-freezer.md`
  - `task-verifier.md`
- Raw artifact: `.agent/tasks/skills-migrate-to-agents/raw/post-agents-agent-files.txt`

### AC3

`/workspace/.claude/skills` is a symlink to `/workspace/.agents/skills` (or equivalent relative symlink target).

Result: PASS

Evidence:

- `readlink .claude/skills` => `../.agents/skills`
- `ls -ld .claude/skills` shows a symlink
- Raw artifacts:
  - `.agent/tasks/skills-migrate-to-agents/raw/readlink-claude-skills.txt`
  - `.agent/tasks/skills-migrate-to-agents/raw/symlink-ls.txt`

### AC4

`/workspace/.claude/agents` is a symlink to `/workspace/.agents/agents` (or equivalent relative symlink target).

Result: PASS

Evidence:

- `readlink .claude/agents` => `../.agents/agents`
- `ls -ld .claude/agents` shows a symlink
- Raw artifacts:
  - `.agent/tasks/skills-migrate-to-agents/raw/readlink-claude-agents.txt`
  - `.agent/tasks/skills-migrate-to-agents/raw/symlink-ls.txt`

### AC5

Existing `.agents/skills` content remains present after migration.

Result: PASS

Evidence:

- Existing entries such as `ark-sync`, `ark-test-env`, `kotlin-android`, `offline-first`, `using-superpowers`, `vercel-react-native-skills`, and `vue-best-practices` are still present in post-migration `.agents/skills` listing.
- Raw artifacts:
  - `.agent/tasks/skills-migrate-to-agents/raw/pre-agents-skill-dirs.txt`
  - `.agent/tasks/skills-migrate-to-agents/raw/post-agents-skill-dirs.txt`

## Verification Notes

A fresh verification script compared pre-migration `.claude` listings against post-migration `.agents` listings and resolved symlink targets. All acceptance criteria passed.
