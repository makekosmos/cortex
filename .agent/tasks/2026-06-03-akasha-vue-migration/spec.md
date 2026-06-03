# Akasha Vue migration

## Goal

Replace in-repo GPUI Akasha with a regular Vue extension and archive the old
GPUI code in a private standalone repository.

## Acceptance

- Private `akasha-gpui` repository contains the old Rust/GPUI Akasha sources.
- `apps/akasha` is removed from the Kosmos workspace.
- `extensions/akasha` is a Vue extension (`kind: "vue"`) built by the existing
  extension pipeline.
- The Vue reader can open a local `.epub`, render spine chapters in order, show
  a table of contents, change font size/family, and persist reader-local state.
- Kosmos docs describe Akasha as a Vue extension, not a native GPUI app.

## Verification

- `bun run --cwd shell build:extensions`
- `bun run --cwd shell typecheck`
- `bun run docs:sync`
- `bun run docs:check`
