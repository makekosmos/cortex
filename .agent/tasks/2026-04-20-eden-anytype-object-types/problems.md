# Problems

## Resolved: Vite config loading failed under the default bundled config loader

### Original Symptom
- `bun run build` failed while loading `vite.config.ts` with `spawn EPERM`

### Fix
- replaced `vite.config.ts` with `vite.config.mjs`
- updated Eden package scripts to use `-c vite.config.mjs --configLoader native`

### Result
- `bun run build` now passes in this environment

## Remaining Environment Limitation

### Symptom
- `npx playwright test ...` still fails before test execution starts because the runner cannot spawn worker processes in this environment

### Assessment
- This does not block task completion anymore.
- Final verification is covered by successful build/typecheck/Rust checks plus deterministic source/presentation verification.
