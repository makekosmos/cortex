# Command Results

## Targeted checks

- `bun run --cwd apps/delphi/ts test`: PASS on escalated rerun, 8 files / 100 tests.
- `bun run --cwd apps/delphi/ts build`: PASS on escalated rerun.
- `bun run --cwd apps/eden/ts test:ark-migration`: PASS.
- `bun run --cwd apps/eden/ts build`: PASS.
- `bun run --cwd packages/kosmos-ark typecheck`: PASS.

## Full smoke

- `bun run ark:smoke`: PASS.

Smoke covered:

- ARK app write boundary guard
- ARK core Rust tests
- usage-tracker Rust tests
- `@kosmos/ark` typecheck
- Arrancador full unit tests and typecheck
- Eden ARK migration test
- Eden build
- Eden typed-note Playwright e2e
- Dashboard smoke seed and analytics using a task-local smoke DB

## Diff hygiene

- `git diff --check`: PASS with line-ending warnings only.

## Search checks

- `git ls-files apps/delphi/ts/sidecar`: no tracked files.
- `rg 'delphi-db|build:sidecar|legacy sidecar' apps/delphi docs TODO.md`: no runtime fallback references remain.
- `rg 'load_entry|search_entries|get_note_type_by_id' apps/eden/ts/main/store.ts`: no normal entry/type/search fallback calls remain.
