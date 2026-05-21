# Task: Arrancador visuals token and content spacing alignment

## Goal

Align Arrancador's shell colors with `packages/kosmos-visuals/theme/css-variables.css` and align the main content edge insets with the desktop Delphi shell conventions.

## Acceptance Criteria

- AC1: Arrancador imports and uses the shared `@kosmos/visuals` CSS variable definitions for app background, sidebar background, borders, and content surface colors instead of maintaining divergent local root values.
- AC2: Arrancador desktop shell background and content surface background resolve to the same token relationships used by `kosmos-visuals` and Delphi desktop shell.
- AC3: Arrancador main page content wrapper uses Delphi-style page edge insets, matching Delphi's `--spacing-page` behavior on desktop and mobile.
- AC4: `bun run typecheck` and `bun run test` pass in `apps/arrancador`.
