# Eden performance overlay

## Goal

Make narrow book pages use the available width and add lightweight FPS diagnostics to Eden's existing `Shift+G` debug grid.

## Scope

- Fix the conflicting narrow-book width rule.
- Show live FPS while the layout grid is enabled.
- Keep a bounded session log of meaningful FPS drops with the best browser-provided cause.
- No new dependency, IPC, persistent database schema, or ARK change.

## Acceptance criteria

- **AC1.** At viewport widths up to 700 CSS pixels, a book cover visual has no maximum-width cap and fills the book header column.
- **AC2.** `Shift+G` shows the existing grid and a live FPS value; toggling it off hides both without intercepting pointer input.
- **AC3.** FPS drops below 75% of the observed session peak are rate-limited and recorded in a bounded in-memory log with `long-task`, `page-hidden`, or `unknown` attribution. Long-task duration is included when available.
- **AC4.** The animation frame and PerformanceObserver are disconnected when Eden unmounts.
- **AC5.** Targeted unit/browser tests, Eden build, formatting, and visual verification pass.
