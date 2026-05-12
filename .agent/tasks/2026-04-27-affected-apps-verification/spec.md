# Task: Verify affected applications

## Context

Recent ARK work touched shared runtime APIs, Arrancador, Dashboard, Eden, Delphi,
and documentation/smoke scripts. The affected applications should be checked
with fresh commands against the current workspace.

All automated verification must use isolated test/smoke databases or temporary
app-data paths only. No check may target a main/user ARK database.

## Acceptance Criteria

- AC1: Shared ARK runtime checks pass.
- AC2: Arrancador typecheck/unit/packaged smoke pass with isolated data.
- AC3: Dashboard typecheck/Electron smoke pass with isolated data.
- AC4: Eden ARK migration/build/e2e smoke path passes with isolated data.
- AC5: Delphi shared ARK task smoke/e2e path passes with isolated data.
- AC6: Evidence records commands, results, and any environment limitations.
