# Verification Problems

## Renderer Build Blocker

- Command: `bun run build:renderer:vue`
- Result: fails before renderer compilation while loading Vite/Tailwind native dependencies.
- Evidence: `.agent/tasks/2026-04-20-arrancador-vue-statistics-route/raw/build-renderer-vue.txt`
- Current error shape:
  - `Could not load ... @tailwindcss/oxide-win32-x64-msvc ... stream did not contain valid UTF-8`
  - `[plugin externalize-deps] Error: spawn EPERM`

## Assessment

- The failure happens in the existing Vue renderer build/tooling path, not in the statistics feature code under `apps/arrancador/src-vue/**`.
- The new statistics route is still type-safe and behavior-verified through the dedicated Vue Vitest config plus `bun run typecheck`.
- The smallest safe diff for this task is to document the blocker rather than modify the shared build pipeline outside the owned scope.
