# Command Results

- `bun run --cwd packages/kosmos-ark typecheck`: PASS.
- `bun run --cwd apps/arrancador test`: PASS, 47 files and 161 tests.
- `bun run --cwd apps/arrancador typecheck`: PASS.
- `bun run --cwd apps/eden/ts test:ark-migration`: PASS, object migration script returned `status: ok`.
- `bun run build` from `apps/eden/ts`: PASS. Warnings only: large chunk and deprecated `inlineDynamicImports`.
- `bunx playwright test tests/app.spec.ts --config playwright.config.ts --grep "custom note type"` from `apps/eden/ts`: PASS, 1 test. Console warnings were Electron CSP/resource warnings already present in the app.
- `cargo test --manifest-path packages/ark-core/rust/Cargo.toml`: PASS, 113 lib tests, 7 rpc tests, integration/doc tests passed.
- `cargo test --manifest-path services/usage-tracker/Cargo.toml`: PASS, 4 tests.
- `node --experimental-strip-types apps/dashboard/scripts/seedSmokeDb.ts --db-path .agent/tasks/2026-04-26-ark-app-completion/smoke/dashboard/smoke-dashboard.db`: PASS.
- `node --experimental-strip-types apps/dashboard/scripts/smokeAnalytics.ts --db-path .agent/tasks/2026-04-26-ark-app-completion/smoke/dashboard/smoke-dashboard.db`: PASS, sessions `30`, top app `Odyssey Browser`, recent sessions `10`.
- `git diff --check`: PASS with CRLF warnings only.
