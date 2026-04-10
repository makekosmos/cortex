# Evidence: remove stale CLAUDE.md references and update tooling to AGENTS.md

## Summary
Cleaned remaining active `CLAUDE.md` references in repo docs, updated workflow skills/templates/tooling to treat `AGENTS.md` as the canonical guide file, switched repo-task-proof-loop subagent installation to the canonical `.agents/agents/` directory, and re-ran the package smoke test successfully.

## Acceptance Criteria Results

### AC1
Repository docs outside historical task artifacts no longer refer to `CLAUDE.md` as the active project guide filename where `AGENTS.md` should be used instead.

Result: PASS

Evidence:
- Updated active docs and notes:
  - `TODO.md` now says `AGENTS.md документация`
  - `dd.md` now refers to `delphi/AGENTS.md`
  - `.agents/skills/update-docs/SKILL.md` now instructs maintaining `AGENTS.md`
- Post-change stale-reference search outside `.agent/tasks/` only finds one intentional anti-regression mention in `/workspace/AGENTS.md`:
  - `Prefer updating and maintaining AGENTS.md files rather than reintroducing CLAUDE.md files.`
- Raw artifact:
  - `.agent/tasks/remove-claude-references-and-tooling/raw/post-stale-claude-search.txt`

### AC2
The repo-task-proof-loop tooling under `.agents/skills/repo-task-proof-loop/` no longer requires or prefers `CLAUDE.md` as the active canonical guide for this repository and supports `AGENTS.md` as canonical.

Result: PASS

Evidence:
- `.agents/skills/repo-task-proof-loop/scripts/task_loop.py` now:
  - discovers guidance from `AGENTS.md` and `.agents/rules/*.md`
  - updates only canonical `AGENTS.md`
  - installs both Claude/Codex subagent templates into `.agents/agents/`
- `.agents/skills/repo-task-proof-loop/scripts/verify_package.py` now validates `.agents/agents/*` and `AGENTS.md`
- Smoke test output shows installed files under `.agents/agents/` and guide creation only for `AGENTS.md`
- Raw artifact:
  - `.agent/tasks/remove-claude-references-and-tooling/raw/verify-package.txt`

### AC3
Skill/template/docs files that instruct users to maintain `CLAUDE.md` are updated to `AGENTS.md` where they describe current repo behavior.

Result: PASS

Evidence:
- Updated files include:
  - `.agents/agents/task-spec-freezer.md`
  - `.agents/agents/task-spec-freezer.toml`
  - `.agents/skills/update-docs/SKILL.md`
  - `.agents/skills/repo-task-proof-loop/SKILL.md`
  - `.agents/skills/repo-task-proof-loop/README.md`
  - `.agents/skills/repo-task-proof-loop/references/REFERENCE.md`
  - `.agents/skills/repo-task-proof-loop/references/COMMANDS.md`
  - `.agents/skills/repo-task-proof-loop/references/SUBAGENTS.md`
  - `.agents/skills/repo-task-proof-loop/VERIFICATION.md`
  - `.agents/skills/repo-task-proof-loop/assets/templates/*task-spec-freezer*`
  - `.agents/skills/repo-task-proof-loop/assets/templates/managed-block-agents.md.tmpl`
- Post-change search no longer shows active-maintenance instructions centered on `CLAUDE.md`.
- Raw artifact:
  - `.agent/tasks/remove-claude-references-and-tooling/raw/post-stale-claude-search.txt`

### AC4
Verification scripts/tests for the repo-task-proof-loop skill still pass after the guide-file logic changes.

Result: PASS

Evidence:
- Ran:
  - `python3 .agents/skills/repo-task-proof-loop/scripts/verify_package.py`
- Result JSON ends with `"result": "PASS"`
- Smoke test confirms:
  - `.agents/agents/*.toml` and `.agents/agents/*.md` are created
  - `AGENTS.md` is created in normal and `--guides auto` flows
  - `validate` returns `valid: true`
- Raw artifact:
  - `.agent/tasks/remove-claude-references-and-tooling/raw/verify-package.txt`

### AC5
Historical evidence/spec artifacts under `.agent/tasks/` are not rewritten except for new artifacts created for this task.

Result: PASS

Evidence:
- Production/doc/tooling updates were limited to active repo files under root docs and `.agents/...`
- Existing historical `.agent/tasks/*` artifacts were not edited as part of this task
- New files for this task are limited to:
  - `.agent/tasks/remove-claude-references-and-tooling/spec.md`
  - `.agent/tasks/remove-claude-references-and-tooling/evidence.md`
  - `.agent/tasks/remove-claude-references-and-tooling/evidence.json`
  - `.agent/tasks/remove-claude-references-and-tooling/verdict.json`
  - `.agent/tasks/remove-claude-references-and-tooling/raw/*`

## Notes
- One remaining `CLAUDE.md` string in root `AGENTS.md` is intentional and descriptive, warning against reintroducing the old filename.
- One remaining `.claude/skills/` path mention in repo-task-proof-loop README is intentional compatibility guidance, explaining how to point legacy readers at canonical `.agents/skills/` via symlink.
