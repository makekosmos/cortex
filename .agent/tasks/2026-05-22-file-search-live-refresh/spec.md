# File Search Live Refresh

## Goal

Apply the second File Search feedback pass: preserve file titles before paths
when result rows run out of width, and refresh a typed query as the file index
changes so users do not need to reopen Kepler to see matches.

## Scope

### In

- File result row layout priority between title, path, and kind label.
- Launcher file-search scheduling for non-empty queries while the index is
  filling or watcher updates arrive after the first query.
- Focused renderer smoke evidence and the normal touched-surface verification.

### Out

- Search result pagination.
- MFT/USN fast scanning.
- Backend search event streaming.

## Acceptance Criteria

**AC1.** A file result row gives the title its intrinsic width before the path
column, while the path still ellipsizes and the kind label remains visible.

**AC2.** A non-empty typed File Search query is refreshed without requiring a
launcher hide/show cycle when a later search pass starts returning results.

**AC3.** Shell type/build checks, backend lib tests, ARK write guard, and ARK
smoke pass after the feedback fix.
