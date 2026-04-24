# Arrancador

Electron + Vue desktop launcher with local game management, backups, statistics, and system tooling.

## Development

- `bun install`
- `bun run dev` launches the Electron shell with the Vite 8 renderer dev server and Bun-built Electron main/preload watchers
- `bun run test` runs the renderer test suite
- `bun run build` builds the Electron app
- `bun run dist` packages installers

## Notes

- Renderer code talks to the app through `window.arrancador`.
- Playtime and usage statistics are read through a narrow adapter in the Electron main process from Ark usage data only. Legacy Arrancador playtime is backfilled into Ark on startup and is no longer used as a live fallback source.
- The old embedded activity watcher no longer belongs to Arrancador. Usage capture now lives in the standalone `services/usage-tracker` service.
- React and Tauri are no longer active runtime paths for Arrancador. Use Electron main-process services and bounded Rust sidecars only when native work is required.
- VS Code users mainly need TypeScript and Electron tooling.
