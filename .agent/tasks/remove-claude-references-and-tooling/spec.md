# Task: remove stale CLAUDE.md references and update tooling to AGENTS.md

## Goal
Clean up remaining stale `CLAUDE.md` references in repository docs and update project tooling/automation so agent guide discovery and generated guidance use `AGENTS.md` as the canonical guide file.

## Acceptance Criteria
- AC1: Repository docs outside historical task artifacts no longer refer to `CLAUDE.md` as the active project guide filename where `AGENTS.md` should be used instead.
- AC2: The repo-task-proof-loop tooling under `.agents/skills/repo-task-proof-loop/` no longer requires or prefers `CLAUDE.md` as the active canonical guide for this repository and supports `AGENTS.md` as canonical.
- AC3: Skill/template/docs files that instruct users to maintain `CLAUDE.md` are updated to `AGENTS.md` where they describe current repo behavior.
- AC4: Verification scripts/tests for the repo-task-proof-loop skill still pass after the guide-file logic changes.
- AC5: Historical evidence/spec artifacts under `.agent/tasks/` are not rewritten except for new artifacts created for this task.

## Verification Plan
- Search for stale `CLAUDE.md` references outside `.agent/tasks/` after edits.
- Run the repo-task-proof-loop verification script(s) affected by the change.
- Record outputs and confirm `AGENTS.md`-based guidance behavior.
