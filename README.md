# Kosmos

Kosmos is a monorepo for local-first personal software: a shared ARK data runtime plus focused desktop/mobile surfaces that render or capture specific workflows.

## Core

### [ARK](./core/ark/crates/ark-core/README.md) - local-first data runtime

ARK is the shared Rust + SQLite runtime for long-lived personal data. The canonical desktop binary is `ark-core-rpc`; Electron integrations talk to it through `@kosmos/ark` from Electron main/preload facades instead of writing directly into ARK SQLite tables.

### [Kosmos desktop shell](./platform/desktop/package.json)

The desktop host is the Electron launcher/settings/runtime shell. It owns extension hosting, the command bus bridge, packaging, and the built-in Dashboard view.

## Extensions

- [Eden](./products/eden/manifest.json) - notes and journal editor.
- [Delphi](./products/delphi/manifest.json) - task tracking UI.
- [Arrancador](./incubator/arrancador/manifest.json) - game library, playtime, backups, and ARK game-object integration.
- [Akasha](./incubator/akasha/README.md) - EPUB reader.

Dashboard is built into the shell under `platform/desktop/src/views/Dashboard*.vue` and `platform/desktop/src/dashboard/`. Focus Session is shell-owned in `platform/desktop/electron/focus-session.ts` and renders through the shared Raycast host.

## Current ARK Integration Rule

New Electron integrations should use `@kosmos/ark` and `ark-core-rpc`. Direct writes into ARK SQLite tables are legacy or migration-only paths; if a process must write directly, it must use `ark_core::db` helpers so sync state is updated consistently.
