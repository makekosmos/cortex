# Kosmos

Kosmos is a monorepo for local-first personal software: a shared ARK data runtime plus focused desktop/mobile surfaces that render or capture specific workflows.

## Core

### [ARK](./crates/ark-core/README.md) - local-first data runtime

ARK is the shared Rust + SQLite runtime for long-lived personal data. The canonical desktop binary is `ark-core-rpc`; Electron integrations talk to it through `@kosmos/ark` from Electron main/preload facades instead of writing directly into ARK SQLite tables.

### [Kosmos desktop shell](./shell/package.json)

The desktop host is the Electron launcher/settings/runtime shell. It owns extension hosting, the command bus bridge, packaging, and the built-in Dashboard view.

## Extensions

Desktop workflow apps live under `extensions/<id>/` and run inside the shell. Horologion source is archived under `extensions/horologion` without an active manifest/package; focus sessions now live in the shell.

- [Eden](./extensions/eden/manifest.json) - notes and journal editor.
- [Delphi](./extensions/delphi/manifest.json) - task tracking UI.
- [Arrancador](./extensions/arrancador/manifest.json) - game library, playtime, backups, and ARK game-object integration.
- [Akasha](./extensions/akasha/README.md) - EPUB reader.

Dashboard is built into the shell under `shell/src/views/Dashboard*.vue` and `shell/src/dashboard/`. Focus Session is shell-owned in `shell/electron/focus-session.ts` and renders through the shared Raycast host.

## Current ARK Integration Rule

New Electron integrations should use `@kosmos/ark` and `ark-core-rpc`. Direct writes into ARK SQLite tables are legacy or migration-only paths; if a process must write directly, it must use `ark_core::db` helpers so sync state is updated consistently.
