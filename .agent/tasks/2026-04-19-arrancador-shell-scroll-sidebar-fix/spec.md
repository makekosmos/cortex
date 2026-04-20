# 2026-04-19 Arrancador Shell Scroll Sidebar Fix

## Context

The current desktop shell in `apps/arrancador` still regresses in three related ways:

- page-level scrolling moves the whole shell, including titlebar and sidebar
- the sidebar can appear visually empty even though its DOM content exists
- the sidebar resize rail can become unreachable or ineffective

The most likely causes are unconfined root scrolling plus unvalidated persisted sidebar width.

## Acceptance Criteria

- AC1: desktop scrolling is confined to the content area instead of scrolling the whole page chrome
- AC2: persisted sidebar width is sanitized on load so the desktop sidebar cannot start in a visually empty or practically unreachable width
- AC3: fresh verification passes (`bun run typecheck`, `bun run test`) and tests cover the sidebar width recovery path
