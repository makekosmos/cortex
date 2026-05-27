# Spec - Akasha standalone installer packaging

## Context

Akasha is a `kind: "native"` Kosmos extension, but native apps should remain usable outside Kosmos. This task adds a local packaging pipeline that produces both a Kosmos `.kext` and a standalone Windows NSIS installer from the same release Rust binary.

## Scope

In scope:

- local packaging command for native extensions;
- Akasha standalone installer generation;
- standalone runtime data dir behavior;
- docs for dual distribution.

Out of scope:

- publishing GitHub releases;
- version bumping;
- code signing;
- bundling Kepler/ARK/runtime services into Akasha standalone.

## Acceptance Criteria

**AC1.** `bun run --cwd shell native:package akasha` builds `cargo build --release -p akasha`, writes an Akasha `.kext`, and attempts to write `Akasha Setup <version>.exe` under `shell/release/native/akasha/`.

**AC2.** The `.kext` packaging path is shared with `ext:publish` instead of having two independent native executable packaging implementations.

**AC3.** The standalone installer script is generated under `shell/.tmp/native-package/akasha/installer.nsi`, installs per-user to `%LOCALAPPDATA%\Programs\Akasha`, creates Start Menu/Desktop shortcuts, registers HKCU uninstall metadata, registers `.epub` opening with `Akasha.exe --open "%1"`, and leaves `%APPDATA%\Akasha` untouched on uninstall.

**AC4.** Standalone Akasha without `--kosmos-user-data-dir` uses `%APPDATA%\Akasha` for `reader-state.json`; Kepler launches continue to use the explicit `--kosmos-user-data-dir` path.

**AC5.** Docs describe native apps as Kosmos-aware but not Kosmos-required, and document `.kext` vs standalone installer outputs and the `native:package` command.
