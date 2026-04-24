# Arrancador Quality Hardening Spec

Task ID: `2026-04-23-arrancador-quality-hardening`

## Goal

Raise Arrancador code quality across architecture, readability, atomicity, and testing by addressing the main blockers found in the review:

- mojibake in user-facing Russian strings and project docs
- default test command missing Electron main tests
- repeated utility logic inside large Vue route pages
- weak measurable gates for future regressions

This pass intentionally avoids broad feature rewrites. It should make the smallest defensible improvements that materially raise maintainability and verification quality.

## Acceptance Criteria

AC1. Mojibake is removed from active Arrancador source and docs touched by this task.

- No active `apps/arrancador` source/doc file contains common mojibake markers such as `Р`, `С`, `вЂ`, or `Рџ` in user-facing Russian text.
- Any remaining matches must be justified as false positives in evidence.

AC2. The default `bun run test` command runs both renderer and Electron main unit tests.

- `apps/arrancador/vitest.config.mjs` includes renderer tests and Electron main `*.test.ts` files.
- Node-only Electron tests run under a compatible environment without requiring jsdom.

AC3. Shared pure game-import/path logic is extracted from large route pages.

- Duplicate path/name/merge helpers currently embedded in `LibraryPage.vue` and `ScanPage.vue` are moved to a shared module.
- Both pages consume the shared module instead of local duplicate implementations.

AC4. Test coverage is added for the extracted shared logic.

- Unit tests cover executable/shortcut path support, filename extraction, clean display-name derivation, and merge-candidate matching.

AC5. Verification is fresh and recorded.

- Run `bun run typecheck`.
- Run `bun run test`.
- Run `bun run biome:check`.
- Attempt `bun run test:e2e`; if blocked by local browser spawn permissions, record the exact blocker.
- Create `evidence.md`, `evidence.json`, and raw command artifacts under this task directory.

## Non-Goals

- Do not redesign the UI.
- Do not rewrite all large route pages in one pass.
- Do not remove unrelated dirty worktree changes.
- Do not claim perfect architecture or complete test coverage beyond the ACs.
