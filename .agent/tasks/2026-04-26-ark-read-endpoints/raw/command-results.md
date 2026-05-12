# Command Results

## Targeted checks

- `bun run --cwd packages/kepler-ark typecheck`: PASS
- `bun run --cwd apps/arrancador test -- ark-usage ark-game-objects`: PASS, 6 files / 21 tests
- `bun run --cwd apps/arrancador typecheck`: PASS
- `bun run ark:guard:writes`: PASS

## Full smoke

- First `bun run ark:smoke`: FAIL at Eden Playwright worker spawn with `spawn EPERM` after earlier smoke steps passed.
- Escalated `bun run ark:smoke`: PASS.

Smoke covered:

- ARK app write boundary guard
- ARK core Rust tests
- usage-tracker Rust tests
- `@kepler/ark` typecheck
- Arrancador full unit tests and typecheck
- Eden ARK migration test
- Eden build
- Eden typed-note Playwright e2e
- Dashboard smoke seed and analytics using a task-local smoke DB

## Diff hygiene

- `git diff --check`: PASS with line-ending warnings only.
