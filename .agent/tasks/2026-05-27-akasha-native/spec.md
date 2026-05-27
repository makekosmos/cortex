# 2026-05-27 — Akasha native extension

## Goal

Add first-class native extension support to Kosmos and ship Akasha, a GPUI-based
EPUB reader, as the first native extension.

## Scope

- Add `kind: "native"` manifest support without breaking existing `vue` and
  `static` extensions.
- Let manifest-declared commands open native extensions through the existing
  launcher command flow.
- Add Akasha as a Rust workspace binary with a minimal GPUI reader UI.
- Package native extensions in `.kext` with a built executable instead of
  `dist/`.
- Keep Akasha v1 local-only: no ARK writes, no sync, no notes/highlights/RAG.

## Acceptance Criteria

**AC1.** Existing Vue/static extension behavior remains compatible: manifests,
declared commands, dev-server resolution, BrowserWindow lifecycle, and e2e
contract flow still work for current extensions.

**AC2.** `kind: "native"` manifests are discoverable by Kosmos, and
manifest-declared `mode: "open"` commands launch the configured executable
through `openExtension(id, route?)`.

**AC3.** Native extension launch is single-instance by default, tracks child
process lifecycle, passes extension id and user-data-dir arguments, and does
not spawn a GUI process in `KOSMOS_HEADLESS=1` / test mode.

**AC4.** Build/publish tooling handles native extensions: Vue extensions are
still built as before, native extension binaries are built with Cargo, and
native `.kext` packages include `manifest.json`, icon/README when present, and
the native executable path declared in the manifest.

**AC5.** Akasha builds as a Rust binary and opens a GPUI window using GPUI
components where practical.

**AC6.** Akasha can parse a fixture EPUB, extract spine text in reading order,
and persist reader state locally in its extension user data directory.

**AC7.** Repository docs and generated agent docs describe `kind: "native"` and
Akasha enough for future agents to work safely.
