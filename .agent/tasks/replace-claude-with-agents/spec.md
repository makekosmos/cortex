# Task: replace CLAUDE.md files with AGENTS.md and clarify AGENTS usage

## Goal
Standardize agent instructions on `AGENTS.md` across the repository by renaming every `CLAUDE.md` to `AGENTS.md`, updating internal references, and expanding the root `AGENTS.md` to clarify that agents should use the nearest project-level `AGENTS.md` files for effective work.

## Acceptance Criteria
- AC1: Every `CLAUDE.md` that exists in the repository before implementation is replaced by an `AGENTS.md` file at the same directory path.
- AC2: No `CLAUDE.md` files remain in the repository after implementation.
- AC3: The root `/workspace/AGENTS.md` explicitly states that agents should use project-level / nearest `AGENTS.md` instructions to work effectively.
- AC4: References inside migrated docs that mention `CLAUDE.md` as the filename are updated to `AGENTS.md` where applicable.
- AC5: Pre-existing `AGENTS.md` files that already existed are preserved and not removed.

## Verification Plan
- Record pre/post lists of `CLAUDE.md` and `AGENTS.md` files.
- Verify zero remaining `CLAUDE.md` files.
- Inspect root `AGENTS.md` for the new explanation.
- Search for stale `CLAUDE.md` references in migrated files.
