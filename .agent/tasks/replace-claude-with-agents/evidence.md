# Evidence: replace CLAUDE.md files with AGENTS.md and clarify AGENTS usage

## Summary
All repository `CLAUDE.md` files were replaced with `AGENTS.md` at the same directory paths, the root `AGENTS.md` was expanded to explain canonical `AGENTS.md` usage and nearest-file guidance, and migrated docs were updated to stop referring to `CLAUDE.md` filenames where applicable.

## Acceptance Criteria Results

### AC1
Every `CLAUDE.md` that existed in the repository before implementation is replaced by an `AGENTS.md` file at the same directory path.

Result: PASS

Evidence:
- Expected migrated paths:
  - `AGENTS.md`
  - `apps/delphi/AGENTS.md`
  - `apps/delphi/kotlin/AGENTS.md`
  - `apps/delphi/swift-archive/AGENTS.md`
  - `apps/eden/AGENTS.md`
  - `apps/elysium/AGENTS.md`
  - `apps/olympia/AGENTS.md`
  - `packages/ark-core/AGENTS.md`
- Verified all expected files exist after migration.
- Raw artifacts:
  - `.agent/tasks/replace-claude-with-agents/raw/pre-claude-files.txt`
  - `.agent/tasks/replace-claude-with-agents/raw/post-agents-files.txt`
  - `.agent/tasks/replace-claude-with-agents/raw/verification.txt`

### AC2
No `CLAUDE.md` files remain in the repository after implementation.

Result: PASS

Evidence:
- Post-migration `CLAUDE.md` count is `0`.
- Raw artifacts:
  - `.agent/tasks/replace-claude-with-agents/raw/post-claude-files.txt`
  - `.agent/tasks/replace-claude-with-agents/raw/verification.txt`

### AC3
The root `/workspace/AGENTS.md` explicitly states that agents should use project-level / nearest `AGENTS.md` instructions to work effectively.

Result: PASS

Evidence:
- Root `AGENTS.md` now includes an `AGENTS.md usage` section stating:
  - `AGENTS.md` is canonical
  - agents should use the nearest relevant `AGENTS.md`
  - subtree `AGENTS.md` files provide project-specific guidance
- Raw artifacts:
  - `/workspace/AGENTS.md`
  - `.agent/tasks/replace-claude-with-agents/raw/verification.txt`

### AC4
References inside migrated docs that mention `CLAUDE.md` as the filename are updated to `AGENTS.md` where applicable.

Result: PASS

Evidence:
- `apps/delphi/AGENTS.md` now points to `swift/AGENTS.md` and `kotlin/AGENTS.md`
- `apps/eden/AGENTS.md` now shows `apps/eden/AGENTS.md` in the structure block
- Raw artifacts:
  - `/workspace/apps/delphi/AGENTS.md`
  - `/workspace/apps/eden/AGENTS.md`
  - `.agent/tasks/replace-claude-with-agents/raw/verification.txt`

### AC5
Pre-existing `AGENTS.md` files that already existed are preserved and not removed.

Result: PASS

Evidence:
- Existing `AGENTS.md` files remain present after migration, including:
  - `AGENTS.md`
  - `apps/eden/ts/AGENTS.md`
- Raw artifacts:
  - `.agent/tasks/replace-claude-with-agents/raw/pre-agents-files.txt`
  - `.agent/tasks/replace-claude-with-agents/raw/post-agents-files.txt`
  - `.agent/tasks/replace-claude-with-agents/raw/verification.txt`
