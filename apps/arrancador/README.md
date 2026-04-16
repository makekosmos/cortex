# Arrancador

Electron + React desktop launcher with local game management, backups, statistics, and system tooling.

## Development

- `bun install`
- `bun run dev` launches the Electron shell with the Vite 8 renderer dev server and Bun-built Electron main/preload watchers
- `bun run test` runs the renderer test suite
- `bun run check:rust` runs the legacy Tauri/Rust checks in `src-tauri`
- `bun run build` builds the Electron app
- `bun run dist` packages installers

## Notes

- Renderer code talks to the app through `window.arrancador`.
- Playtime and usage statistics are read through a narrow adapter in the Electron main process. Ark DB is the preferred source; legacy local tables remain as transitional fallback.
- The old embedded activity watcher no longer belongs to Arrancador. Usage capture now lives in the standalone `services/usage-tracker` service.
- VS Code users mainly need TypeScript, Rust-analyzer, and Electron tooling.
