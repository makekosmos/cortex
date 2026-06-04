# Raycast compatibility runtime foundation

## Classification

`FULL_LOOP` — task touches extension host contracts, command lifecycle, capability/security boundaries, and a new compatibility package.

## Goal

Add the first safe, testable Raycast-compatible vertical slice for Kosmos without weakening existing Vue extension security:

- Raycast-style `package.json` manifests can be discovered as `kind: "raycast"` extensions.
- Raycast `no-view` commands can be surfaced in the launcher and executed through a dedicated runner.
- A private `@raycast/api` shim exists with the first API/components needed by the target slice.
- Untrusted user-installed Raycast command code is not executed in the Electron main process until an isolated runtime exists.

## Scope

In scope:

- Raycast manifest parser for `package.json`.
- Mapping Raycast `commands[].mode = "no-view"` to launcher-declared command records.
- A capability-aware `no-view` command runner for trusted dev/bundled sources.
- `@raycast/api` shim skeleton: `List`, `Detail`, `ActionPanel`, `Action`, `Toast`, `showToast`, `showHUD`, `confirmAlert`, `Clipboard`, `LocalStorage`, `Cache`, `getPreferenceValues`, `environment`, `launchCommand`, and `useNavigation`.
- Unit tests for parser, API model, storage/runtime calls, and trusted-source execution guard.

Out of scope:

- Full React renderer, TSX bundler, HMR, Store compatibility, OAuth, AI, BrowserExtension, WindowManagement, MenuBarExtra, Grid/Form rich parity, and untrusted third-party JS sandboxing.
- Publishing/release/version bump.

## Acceptance Criteria

**AC1.** `@raycast/api` exists as a private workspace package and exports the Phase 1 API surface needed by the example target: `List`, `List.Item`, `List.Section`, `Detail`, `ActionPanel`, `Action.CopyToClipboard`, `Action.Push`, `Toast`, `showToast`, `showHUD`, `Clipboard`, `LocalStorage`, `Cache`, `getPreferenceValues`, `environment`, `launchCommand`, and `useNavigation`.

**AC2.** Raycast-style `package.json` manifests are parsed and validated without requiring Kosmos `manifest.json`, including extension id/name/title, version, description, command `name/title/mode`, preferences, and `kosmos.permissions`.

**AC3.** Existing extension discovery recognizes a directory with Raycast `package.json` as `kind: "raycast"` and maps `no-view` commands into the existing launcher declared-command pipeline with ids `${extensionId}:${commandName}`.

**AC4.** Invoking a Raycast `no-view` declared command uses a dedicated runner, passes launch props with `LaunchType.UserInitiated`, and exposes runtime-backed `Clipboard`, `LocalStorage`, `Cache`, feedback, preferences, and `launchCommand`.

**AC5.** The runner refuses to execute user-installed Raycast command code in the Electron main process, while trusted `dev` / `bundled` sources are allowed. This prevents a new arbitrary-code bypass of the current permission model.

**AC6.** Existing Vue/static/native manifest behavior remains compatible: `kind`, ordinary `manifest.json` loading, command `open/action` modes, and extension permission checks continue to typecheck and pass relevant tests.

**AC7.** Verification evidence records `bun test` unit coverage for the new parser/API/runner tests, `bun run shell:typecheck`, `bun run ark:guard:writes`, and `bun run ark:smoke` or records any environment failure with raw output.
