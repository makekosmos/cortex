# File Search Result Polish

## Goal

Fix the first feedback pass on Kepler File Search results: keep backend result
names useful even for malformed/stale index rows, avoid starving relevant file
matches before ranking, and make file rows readable in the launcher.

## Scope

### In

- Backend search coverage for blank indexed names and candidate selection used by
  File Search ranking.
- A focused launcher file result component with a name fallback, compact
  document-style icon treatment, and long path truncation.
- Verification for touched Rust and shell surfaces.

### Out

- MFT/USN fast scanning.
- Content search and folder results.
- File-type-specific native shell icons.

## Acceptance Criteria

**AC1.** File Search backend returns a usable file result title when an indexed
row has a blank stored name and tests cover the fallback path.

**AC2.** Backend search gives the ranker a broad enough candidate set that
relevant filename matches are not lost behind a small pre-ranking window, with a
focused regression test.

**AC3.** Launcher file rows render through a dedicated component that keeps a
document-style file icon, a visible fallback title, and an ellipsized long path
without pushing the result kind outside the row.

**AC4.** Verification passes for focused backend tests, backend lib tests, shell
type/build checks, ARK write guard, and ARK smoke.
