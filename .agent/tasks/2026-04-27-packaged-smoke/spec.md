# Task: Packaged smoke checks for Arrancador and Dashboard

## Context

The ARK migration work has broad unit, typecheck, and smoke coverage, but the
remaining verification gap is packaged/build smoke coverage. Dashboard already
has an isolated Electron smoke path. Arrancador can build/package, but its
existing E2E test is renderer/mock based and does not launch the Electron main
runtime with isolated app-data.

All automated verification must use isolated test/smoke databases or temporary
app-data paths only. No smoke run may target a main/user ARK database or normal
Electron userData directory.

## Acceptance Criteria

- AC1: Arrancador has a Windows-friendly isolated packaged/build smoke script.
- AC2: The Arrancador smoke script sets temporary `APPDATA`, `LOCALAPPDATA`,
  and `ARK_DB_PATH` before launching Electron.
- AC3: The Arrancador smoke script verifies the app starts and exits without
  requiring user interaction.
- AC4: Dashboard packaged/Electron smoke is documented and run through its
  existing isolated smoke path.
- AC5: Proof evidence records command results and confirms test database
  isolation.
