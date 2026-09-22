# KOS-131 — Host: Electron vs GPUI

**Decision: Electron stays the product Host; GPUI ships as an
Engine-connected companion shell (dual-run), not a replacement.**

## Why not transition now

1. **OS integration lives in Electron today.** Login items
   (`app.setLoginItemSettings`), the auto-updater feed, native file dialogs,
   BrowserWindow management for hosted packages, dictation pill overlay,
   focus-mode windows — all are Electron main-process code paths. A GPUI Host
   would need each of these rebuilt as a Windows-native layer; that is its own
   epic, not a side effect of KOS-130.
2. **GPUI on Windows is unproven in this codebase.** Both `agenda-gpui` and
   `manager-gpui` were verified on Linux/Xvfb only. `gpui-kit 0.6.2` renders
   via Blade/D3D11 on Windows, but input, IME, and window-chrome behavior need
   a real Windows QA pass before it can own the shell.
3. **The risk profile differs by layer.** Engine (`kepler-backend` +
   `ark-core`) is platform-agnostic and already proven. The shell layer is
   where Electron earns its keep: packaging (electron-builder), signing,
   updater plumbing, crash reporting hooks. Replacing it buys smaller binaries
   and one less JS runtime — real but not urgent benefits.

## What GPUI already covers

`manager-gpui` demonstrates the full Manager surface over `/v1/rpc`: all 11
Vue views, Engine discovery via `engine.lock.json`, disclosure/confirm flows,
Engine-down UX. The pattern generalizes — any Vue view that talks to Engine
can be ported with the same worker-thread + slot-store architecture.

## Roadmap (dual-run → optional transition)

- **Now (KOS-127/KOS-134/KOS-137):** Electron Host ships the product. Since
  KOS-134 the Windows package embeds `manager-gpui` as `components/manager`
  (`Kosmos Manager.exe`) — the Vue `manager/` source stays in the repo for
  rollback but is no longer the packaged default. Since KOS-137 the package
  also embeds `agenda-gpui` as `components/agenda` (`Kosmos Agenda.exe`),
  pinned via `desktop/component-pins.json` and launched through the shell
  command «Открыть Agenda (GPUI)», the Start Menu «Kosmos Agenda» shortcut,
  or the GPUI Manager About view; the Vue Agenda package stays installable
  as fallback.
- **Follow-up A — Windows GPUI bring-up:** build `manager-gpui` on Windows,
  QA input/IME/titlebar/HiDPI, fix `windows_subsystem` + traffic-light
  assumptions (Linux-only conveniences are already isolated in `main.rs`).
- **Follow-up B — GPUI Host spike (timeboxed):** prove launch leases +
  `openHostedPackage` equivalents from GPUI: window management for hosted
  packages is the hardest remaining Electron dependency.
- **Follow-up C — updater/installer story:** a GPUI Host still needs
  distribution; evaluate tauri-style bundling vs. keeping electron-updater
  for a thin companion process.
- **Revisit decision** after A+B land: if both succeed, transitioning the
  Host to GPUI removes the entire Electron dependency chain; if either
  stalls, dual-run is already a complete product state.

## What stays regardless

- Engine is the only state owner; every shell (Electron, GPUI, future)
  uses `/v1/rpc` + lock-file discovery. No shell opens SQLite.
- The one-revision BOM (`release-bom.mjs`) covers Host + kepler-backend +
  ark-core-rpc from a single commit — independent of shell choice.
