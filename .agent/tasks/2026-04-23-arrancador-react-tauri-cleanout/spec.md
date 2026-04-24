# Arrancador React/Tauri cleanout

## Goal

Fully remove remaining React and Tauri traces from the active Arrancador project after the Electron + Vue migration, including stale generated assets, stale configuration, and stale dependency surfaces.

## Acceptance criteria

- AC1: `apps/arrancador/package.json` declares no React, React DOM, React Vite plugin, Tauri, or Tauri API dependencies/devDependencies/scripts.
- AC2: `apps/arrancador` contains no active `src-tauri` directory and no `.tsx` or `.jsx` files outside ignored build/dependency output.
- AC3: Arrancador runtime/config source contains no active imports or references to React, React DOM, `@vitejs/plugin-react`, `@tauri-apps/api`, `__TAURI__`, or Tauri config/runtime files.
- AC4: TypeScript, Vite, Vitest, Tailwind, and Biome configuration no longer advertise JSX/TSX or ignored Tauri source paths that are not part of the active app.
- AC5: Automated architecture tests enforce the React/Tauri cleanout.
- AC6: Fresh verification passes for typecheck, unit tests, and Biome checks. E2E is attempted and any local environment blocker is recorded.
