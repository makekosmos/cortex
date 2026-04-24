# Problems And Fixes

## Resolved

### P1: `biome:check` failed on generated output and source diagnostics
- Initial `biome:check` reported generated/build output noise plus source diagnostics.
- Fixed by excluding generated folders in `apps/arrancador/biome.jsonc`, updating the Biome schema version, applying safe Biome fixes, and removing the remaining source errors.
- Reverified with `bun run biome:check`.

### P2: Renderer build was flaky after the native rebuild step
- `bun run build` and redirected `bun run build:renderer` could fail on Windows while loading `@tailwindcss/oxide` through Vite's default config bundler.
- Fixed by changing `build:renderer` to `vite build --configLoader native`, matching the existing Vitest native config loader approach.
- Reverified with `bun run build:renderer` and full `bun run build`.

### P3: Arrancador package lock metadata still referenced removed React/Tauri roots
- `bun install` could not run in the sandbox because Bun could not write to the system tempdir (`AccessDenied`).
- Manually synchronized the Arrancador workspace dependency section and removed unreferenced Tauri package entries from `bun.lock`.

### P4: Legacy Tauri backend source still existed
- Recursive shell deletion was blocked by the execution policy.
- Removed the text/config Rust Tauri backend files with `apply_patch`.
- Binary icon/resource assets under `apps/arrancador/src-tauri/` remain because `apply_patch` cannot delete invalid-UTF8/binary files and shell deletion is blocked.

### P5: Preload IPC bridge trusted TypeScript-only channel narrowing
- The bridge accepted arbitrary runtime strings even though source callers were typed.
- Added runtime whitelists for allowed invoke channels and event channels in `electron/shared/ipc.ts`.
- Reverified typecheck, tests, lint, renderer build, main build, preload build, and full build.
