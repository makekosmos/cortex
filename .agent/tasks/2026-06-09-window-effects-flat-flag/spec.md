# 2026-06-09 — Window effects flat flag

## Context

Evidence for `.agent/tasks/2026-06-08-ws-hot-path-performance/` claimed a flat window-effects baseline via `KOSMOS_WINDOW_EFFECTS=flat`, but desktop Electron runtime did not consume that env. Launcher only accepted the legacy `KEPLER_BG_MATERIAL`, and other BrowserWindow surfaces used hardcoded materials or per-extension manifest values.

## Scope

In scope:

- Add one Electron main-process resolver for `KOSMOS_WINDOW_EFFECTS=flat|mica|acrylic`.
- Preserve legacy `KEPLER_BG_MATERIAL=mica|none|acrylic` behavior when `KOSMOS_WINDOW_EFFECTS` is unset.
- Apply the global resolver to launcher, settings, install extension, extension host, focus widget, dictation pill, and focus overlay windows.
- Add a regression test for env parsing and precedence.

Out of scope:

- Re-running the performance benchmark.
- Changing renderer visual design, transparency, titlebar, focus-mode lifecycle, dictation lifecycle, or extension manifest schema.

## Acceptance Criteria

**AC1.** `KOSMOS_WINDOW_EFFECTS=flat|mica|acrylic` is parsed in runtime code, where `flat` maps to Electron `backgroundMaterial: "none"`.

**AC2.** The global flag affects all requested surfaces: `main.ts` launcher, `settings-window.ts`, `install-extension-window.ts`, `extension-host.ts`, `focus-widget.ts`, `dictation-pill.ts`, and `focus-overlay.ts`.

**AC3.** Existing `KEPLER_BG_MATERIAL=mica|none|acrylic` remains supported for launcher/backdrop compatibility when `KOSMOS_WINDOW_EFFECTS` is absent.

**AC4.** A regression test fails if `KOSMOS_WINDOW_EFFECTS=flat` is ignored or if legacy env takes precedence over the new global flag.

**AC5.** Typecheck and relevant Electron tests pass, and docs/postmortem record the corrected contract.
