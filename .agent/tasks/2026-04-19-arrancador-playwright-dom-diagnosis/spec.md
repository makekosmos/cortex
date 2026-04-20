# 2026-04-19 Arrancador Playwright DOM Diagnosis

## Context

The current `apps/arrancador` desktop shell still feels broken to the user in two ways:

- startup DOM paint is very slow in dev
- the sidebar is reported as visually empty even when its DOM nodes exist

The user explicitly requested Playwright CLI-based diagnosis. This task uses Playwright artifacts to reproduce the renderer state, then applies the smallest safe code fix for any confirmed shell regression.

## Acceptance Criteria

- AC1: a Playwright CLI diagnostic run captures current renderer timings, sidebar screenshot evidence, and computed sidebar state artifacts
- AC2: the desktop shell no longer has a credible path where the sidebar appears empty because of hidden/collapsed shell state without an obvious recovery affordance
- AC3: fresh verification passes for the changed code (`bun run typecheck`, `bun run test`), and Playwright artifacts are stored under this task directory
