# 2026-06-24 Desloppify Extension Installer Intermediate Cleanup

## Classification

FULL_LOOP.

## Goal

Remove low-risk `UNNECESSARY_INTERMEDIATE` findings from the desktop extension
installer without changing install/list behavior.

## Context

`platform/desktop/electron/extension-installer.ts` had two local variables that
were returned immediately:

- `finalPreview` after install
- `merged` installed extension list

Both can be returned directly while keeping the same awaited operations and
dedupe ordering.

## Scope

In scope:

- `platform/desktop/electron/extension-installer.ts`
- This task's evidence files

Out of scope:

- Extension install/revert semantics
- Extension host architecture
- Release/distribution behavior

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after full-scan JSON outputs and a summary of the
desloppify delta.

**AC2. Behavior is preserved.**
The installer still returns `previewDir(target)` after install, and installed
extensions are still ordered as dev entries followed by non-shadowed installed
entries.

**AC3. Relevant checks are recorded.**
Typecheck, shell build, nearby tests, and the full scan run are recorded,
including any unrelated failing test expectation.
