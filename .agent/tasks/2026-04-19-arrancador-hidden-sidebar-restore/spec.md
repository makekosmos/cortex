# 2026-04-19 Arrancador Hidden Sidebar Restore

## Context

`apps/arrancador` keeps desktop sidebar visibility in persisted config. When `arrancador-sidebar-config.hidden` is `true`, the desktop sidebar collapses to zero width and all sidebar buttons disappear with it. Users need a visible desktop affordance to restore the sidebar without relying on hidden state knowledge.

## Acceptance Criteria

- AC1: when the desktop sidebar is hidden, the layout shows a visible restore control outside the collapsed sidebar
- AC2: activating the restore control unhides the desktop sidebar and exposes its navigation/search controls again
- AC3: the hidden-sidebar restore path is covered by tests and passes fresh `bun run typecheck` and `bun run test`
