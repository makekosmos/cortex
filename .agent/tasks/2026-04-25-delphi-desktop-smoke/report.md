# Delphi desktop smoke/build report

Date: 2026-04-25
Workspace: `D:\Personal\Hobby\Coding\kepler`
Project: `apps/delphi/ts`

## Package scripts found

From `apps/delphi/ts/package.json`:

- `build:sidecar`: `cargo build --release --manifest-path ../../../packages/ark-core/rust/Cargo.toml --bin ark-core-rpc`
- `build:sidecar:dev`: `cargo build --manifest-path ../../../packages/ark-core/rust/Cargo.toml --bin ark-core-rpc`
- `dev`: `bun run build:sidecar:dev && vite --configLoader native`
- `build`: `bun run build:sidecar && tsc && vite build --configLoader native && node scripts/run-electron-builder.mjs`
- `build:web`: `BUILD_TARGET=web tsc && BUILD_TARGET=web vite build --configLoader native`
- `test`: `vitest run`
- `test:watch`: `vitest`
- `preview`: `vite preview --configLoader native`
- `package`: `electron-builder --dir`
- `dist`: `electron-builder`
- `lint`: `oxlint -c .oxlintrc.json src/`
- `lint:fix`: `oxlint -c .oxlintrc.json --fix src/`
- `format`: `oxfmt src/`
- `format:check`: `oxfmt --check src/`
- `test:e2e`: `playwright test`
- `e2e`: `playwright test`
- `e2e:ui`: `playwright test --ui`

## Commands run

| Command | Result | Notes |
| --- | --- | --- |
| `bun run test` | PASS | Vitest: 7 test files passed, 93 tests passed, duration 32.64s. |
| `.\node_modules\.bin\tsc.cmd --noEmit` | FAIL_TO_START | Local `.bin` contains `tsc.exe`/`tsc.bunx`, not `tsc.cmd`; PowerShell could not find the command. Re-run below with `tsc.exe`. |
| `.\node_modules\.bin\tsc.exe --noEmit` | PASS | TypeScript typecheck completed with exit code 0 and no output. |
| `bun run build` | FAIL | `build:sidecar` completed after waiting for a Cargo build lock; renderer build completed; Electron production build failed in `vite-plugin-electron:prod`. |
| `bun run build:web` | PASS | Web build completed; warning only: `/fonts/zed-mono-extended.ttf` remains unresolved until runtime, plus `vite-plugin-checker` timing warning. |

## Key failure

`bun run build` failed while building Electron main:

```text
Error: [vite]: Rolldown failed to resolve import "@kepler/ark" from "D:/Personal/Hobby/Coding/kepler/apps/delphi/ts/electron/main.ts".
This is most likely unintended because it can break your application at runtime.
If you do want to externalize this module explicitly add it to
`build.rollupOptions.external`
```

Failure metadata:

- Plugin: `vite-plugin-electron:prod`
- Hook: `closeBundle`
- Exit code: 1
- No network requirement observed before failure.

## Smoke status

`test:e2e` / `e2e` were not run because `e2e/shared-ark-task.spec.ts` uses Playwright `_electron.launch`, which launches Electron windows. That violates the requested "without GUI" constraint.

## Overall

- Headless unit tests: PASS
- Headless typecheck: PASS
- Desktop production build: FAIL
- Web build: PASS
- GUI smoke/e2e: SKIPPED by constraint
