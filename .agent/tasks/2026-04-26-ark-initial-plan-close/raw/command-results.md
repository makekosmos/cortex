# Command Results

- `bun run ark:guard:writes`: PASS, `ARK write boundary guard passed`.
- `bun run ark:smoke`: PASS.
  - ARK core Rust tests: PASS.
  - usage-tracker Rust tests: PASS.
  - `@kepler/ark` typecheck: PASS.
  - Arrancador unit tests: PASS, 47 files and 161 tests.
  - Arrancador typecheck: PASS.
  - Eden ARK migration script: PASS.
  - Eden build: PASS with existing chunk-size and `inlineDynamicImports` warnings.
  - Eden typed-note e2e: PASS, 1 test.
  - Dashboard smoke seed: PASS.
  - Dashboard smoke analytics: PASS, sessions `30`, top app `Odyssey Browser`, recent sessions `10`.
- `git diff --check`: PASS with CRLF warnings only.
