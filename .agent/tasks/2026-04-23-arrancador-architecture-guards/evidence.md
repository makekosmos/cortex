# Evidence

Task ID: `2026-04-23-arrancador-architecture-guards`

## Verification Result

Overall: `PASS`

## Acceptance Criteria

AC1. Renderer boundary regressions are guarded.

Result: `PASS`

Evidence:
- Added `apps/arrancador/src-vue/test/architecture-boundaries.test.ts`.
- The test scans Vue renderer runtime files and fails if they import Electron, Node builtins, Electron main modules, or Tauri APIs/globals.

AC2. Deprecated runtime/test dependencies are guarded.

Result: `PASS`

Evidence:
- The architecture guard scans active Arrancador source for React runtime imports, `bun:test`, Tauri APIs, and Tauri globals.
- `bun run test` passes with the guard enabled.

AC3. The guard is part of the default test suite.

Result: `PASS`

Evidence:
- The guard lives under `src-vue/test`, which is included in the default renderer Vitest project.
- `bun run test`: `14 passed (14)`, `38 passed (38)`.

AC4. Verification is fresh and recorded.

Result: `PASS`

Evidence:
- `bun run typecheck`: pass.
- `bun run test`: pass.
- `bun run biome:check`: pass.
- `bun run test:e2e`: attempted; app build completed, Playwright execution blocked by local `spawn EPERM` when launching worker/browser process. Blocker is recorded in `raw/test-e2e.txt`.

AC5. Proof artifacts are recorded.

Result: `PASS`

Evidence:
- Raw artifacts:
  - `raw/typecheck.txt`
  - `raw/test.txt`
  - `raw/biome-check.txt`
  - `raw/test-e2e.txt`

## Notes

- This pass does not change runtime behavior. It adds automated guardrails so Arrancador stays on the intended Electron + Vue architecture.
