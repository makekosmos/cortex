# 2026-06-04 raycast-menu-bar-host

## Context

Raycast package manifests already parse `commands[].mode = "menu-bar"` and the shim exports `MenuBarExtra`, but the extension host still routed menu-bar commands as regular view commands. This slice adds a trusted host-rendered `MenuBarExtra` vertical path without implementing a native tray/menu-bar lifecycle.

## Scope

In scope:

- Map Raycast `menu-bar` commands to a distinct declared `raycast-menu-bar` mode.
- Let the trusted Raycast runner execute `menu-bar` commands and normalize a `MenuBarExtra` snapshot.
- Render `MenuBarExtra.Section`, `MenuBarExtra.Item`, and `MenuBarExtra.Submenu` in the shell Raycast host.
- Execute `MenuBarExtra.Item.onAction` callbacks through guarded session IPC.
- Add regression tests, docs, proof evidence, and a visual screenshot.

Out of scope:

- Real OS tray/menu-bar residency.
- Background refresh intervals for menu-bar commands.
- Untrusted Raycast extension execution.

## Acceptance Criteria

AC1. Focused Raycast tests pass and cover trusted `menu-bar` command execution plus `MenuBarExtra` host model extraction.

AC2. `bun run shell:typecheck` passes after mode-union and Vue renderer changes.

AC3. A Playwright visual check captures the `MenuBarExtra` host and verifies an item callback.

AC4. `bun run docs:sync` and `bun run docs:check` pass after updating Raycast command-bus/extension docs.

AC5. The full Raycast unit suite plus `ark:guard:writes` and `ark:smoke` pass.

## Verification commands

- `bun test tests\unit\raycast-command-runner.test.ts tests\unit\raycast-view-model.test.ts tests\unit\raycast-manifest.test.ts`
- `bun run shell:typecheck`
- Visual script under `.tmp/visual/2026-06-04-raycast-host/`
- `bun test tests\unit\raycast-api.test.ts tests\unit\raycast-manifest.test.ts tests\unit\raycast-command-runner.test.ts tests\unit\raycast-view-model.test.ts tests\unit\extension-permissions.test.ts`
- `bun run docs:sync`
- `bun run docs:check`
- `bun run ark:guard:writes`
- `bun run ark:smoke`

## Out of Scope Decisions

The first `MenuBarExtra` support is host-rendered in the Raycast window route. Native tray/menu-bar residency needs a separate lifecycle design and proof loop.
