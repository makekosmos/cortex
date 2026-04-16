# AGENTS.md

Project: Arrancador (Electron + React, with legacy Tauri subtree still present)

Purpose
- Desktop launcher for games and local library management.
- Reads usage/playtime data from Ark DB instead of owning the activity tracker process.

Tech stack
- Frontend: React + TypeScript + Vite
- Desktop shell: Electron
- Local persistence: SQLite via `better-sqlite3` in Electron main process
- Legacy subtree: `src-tauri/` still exists, but new usage-tracking work should not be added there

Repo layout
- `src/` Renderer UI
- `electron/main.ts` Electron bootstrap
- `electron/preload.ts` Secure bridge for renderer
- `electron/main/db/` Local database adapters
- `electron/main/services/games.ts` Game CRUD / launch orchestration
- `electron/main/services/ark-usage.ts` Ark DB reader for tracked usage data
- `electron/main/services/playtime-stats.ts` Arrancador-side aggregation over Ark usage data
- `scripts/` Dev/build helpers
- `src-tauri/` Legacy Tauri backend code that still ships for unrelated features

How to work
- Use `rg` for search.
- Prefer small, focused changes with clear reasoning.
- Use UTF-8 for edited files.
- Keep the Electron boundary narrow: renderer calls preload, preload calls explicit main-process handlers.

Common tasks
1. UI changes
   - Update React components in `src/`.
   - Keep data-fetching hooks thin; heavy I/O belongs in Electron main services.
2. Main-process features
   - Add or update handlers in `electron/main.ts`.
   - Put Ark usage reads in `electron/main/services/ark-usage.ts` or adjacent services, not in renderer code.
3. Usage/playtime work
   - Treat Ark DB as the source of truth.
   - Do not reintroduce an in-process tracker, window polling loop, or Arrancador-owned usage SQLite.
4. Legacy cleanup
   - `src-tauri/` and `example/` may still contain historical code; verify current call paths before deleting or reusing anything.

Testing
- `bun run typecheck`
- `bun run build:renderer`
- `bun run build:main`
- `bun run build:preload`
- `bun run test` when touching renderer logic

Notes
- Arrancador now consumes usage data; `services/usage-tracker` owns collection.
- If you change Ark usage queries, validate against the actual Ark schema rather than inventing launcher-local copies.
- Some Tauri-era files remain in the tree; do not assume they are the active runtime path.
