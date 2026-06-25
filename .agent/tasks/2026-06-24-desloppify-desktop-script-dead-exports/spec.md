# 2026-06-24 Desloppify Desktop Script Dead Exports

## Classification

FULL_LOOP.

## Goal

Remove confirmed unused exports from desktop packaging/dev scripts while
preserving script behavior and public script helpers that are imported by other
repo scripts or tests.

## Context

Current scan after prior cleanup:

- Score: 0
- Findings: 501 total
- Severity: critical 0, high 300, medium 130, low 71

`desloppify` still reports many `DEAD_EXPORT` findings. Public contract files
and app typed models have higher false-positive risk, so this slice targets
small script helpers with direct import/reference evidence.

## Scope

In scope:

- `platform/desktop/scripts/repo-extension-roots.mjs`
- `platform/desktop/scripts/extension-package-utils.mjs`
- `platform/desktop/scripts/zip-utils.mjs`
- This task's evidence files

Out of scope:

- Release version changes
- Distribution flow behavior
- Electron installer implementation
- App/runtime/ARK code
- Broad script refactors

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after `desloppify scan --json .` outputs and a summary
of total findings and severity counts.

**AC2. Confirmed dead script exports are removed.**
The scoped `DEAD_EXPORT` findings for internal script helpers are gone after
reference checks confirm no external imports.

**AC3. Active script APIs remain available.**
Script helpers imported by other scripts/tests continue to be exported.

**AC4. Relevant script checks pass.**
Focused desktop script/test checks pass after the change, or any unrelated
blocker is documented with exact command output.
