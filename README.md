# Kepler

Kepler is a monorepo for local-first personal software: a shared ARK data runtime plus focused desktop/mobile apps that render or capture specific workflows.

## Core

### [ARK](./packages/ark-core/README.md) - local-first data runtime

ARK is the shared Rust + SQLite runtime for long-lived personal data. The canonical desktop binary is `ark-core-rpc`; Electron apps should talk to it through `@kepler/ark` from Electron main/preload facades instead of writing directly into ARK SQLite tables.

### [Eden](./apps/eden/README.md) - journal/editor

Eden is the writing and journal surface. It uses its own editor/search sidecar and integrates with ARK for shared objects/sync.

## Helper Apps

Helper apps are thin workflow surfaces around shared data. Their job is quick capture, focused visualization, or domain-specific interaction while ARK owns storage/sync contracts.

### [Delphi](./apps/delphi/README.md)

Task tracking UI. The Electron package uses the shared `ark-core-rpc` binary.

### [Arrancador](./apps/arrancador/README.md)

Game library, playtime, backups, and ARK game-object integration.

### [Dashboard](./apps/dashboard/README.md)

Read-only usage analytics over ARK usage data.

### [Olympia](./apps/olympia/README.md)

Workout tracking.

### [Elysium](./apps/elysium/README.md)

Nutrition tracking.

## Current ARK Integration Rule

New Electron integrations should use `@kepler/ark` and `ark-core-rpc`. `@arksync/node` is compatibility-only. Direct writes into ARK SQLite tables are legacy or migration-only paths; if a process must write directly, it must use `ark_core::db` helpers so sync state is updated consistently.
