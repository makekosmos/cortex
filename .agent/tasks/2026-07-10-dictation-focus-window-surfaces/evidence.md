# Evidence — Dictation and Focus window surfaces

Verified at `2026-07-10T07:26:45Z` against the current working tree.

## AC1 — Dictation topmost contract

**PASS (automated contract).** `showPill()` now restores the `screen-saver` always-on-top level after the headless guard, shows without activation, and calls `moveTop()`.

Command:

```powershell
rtk test bun test platform/desktop/electron/dictation-pill-window.test.ts
```

Result: `1 pass, 0 fail`. Before the fix, the regression test failed because the per-show `setAlwaysOnTop` call was absent.

## AC2 — Focus transparent native backing

**PASS (automated contract).** The Focus widget keeps `transparent: true` and `backgroundColor: "#00000000"`, and no longer imports, configures, or applies Electron background material APIs.

Command:

```powershell
rtk test bun test platform/desktop/electron/focus-widget-window.test.ts
```

Result: `1 pass, 0 fail`. Before the source fix, the regression test failed on the existing `backgroundMaterialOption(...)` / `applyWindowMaterial(...)` path.

## AC3 — Existing Focus lifecycle and IPC

**PASS.** The implementation diff is confined to native window construction; `focus-widget.ts`, IPC registration, Pomodoro/backend state, and hosts-file code are unchanged. Desktop TypeScript still compiles.

Command:

```powershell
rtk bun run --cwd platform/desktop typecheck
```

Result: exit code `0`.

## AC4 — Focused automated checks

**PASS.** Combined current-tree verification:

```powershell
rtk test bun test platform/desktop/electron/focus-overlay-window.test.ts platform/desktop/electron/focus-widget-window.test.ts platform/desktop/electron/dictation-pill-window.test.ts
rtk bun run --cwd platform/desktop typecheck
rtk bunx oxfmt --check platform/desktop/electron/dictation-pill.ts platform/desktop/electron/dictation-pill-window.test.ts platform/desktop/electron/focus-widget-window.ts platform/desktop/electron/focus-widget-window.test.ts platform/desktop/electron/focus-overlay.ts platform/desktop/electron/focus-overlay-window.test.ts docs-site/agents/postmortems.md .agent/tasks/2026-07-10-dictation-focus-window-surfaces/spec.md
rtk git diff --check
```

Results: `3 pass, 0 fail`; typecheck exit `0`; formatting PASS; diff check PASS.

## AC5 — Windows runtime/visual proof

**PASS.** A real visible Windows Electron instance was launched with DevTools closed against a separate fullscreen Chromium process.

- Focus: the striped fullscreen surface remains visible immediately outside the rounded widget corners; no white, black, or opaque rectangular BrowserWindow backing is present.
- Dictation: the pill remained visible above fullscreen Chromium on the first recording and after `cancel -> show` reused the same window. Chromium was explicitly brought to the foreground after each show; the pill still remained above it without taking input focus.
- Native state recorded the visible dictation window with `alwaysOnTop: true` and the expected compact bounds.

Artifacts:

- `.tmp/visual/2026-07-10-dictation-focus-window-surfaces/focus-native-transparency.png`
- `.tmp/visual/2026-07-10-dictation-focus-window-surfaces/focus-active-widget.png`
- `.tmp/visual/2026-07-10-dictation-focus-window-surfaces/focus-widget-closeup.png`
- `.tmp/visual/2026-07-10-dictation-focus-window-surfaces/focus-blocked-app-gradient.png`
- `.tmp/visual/2026-07-10-dictation-focus-window-surfaces/focus-blocked-overlay-example.png`
- `.tmp/visual/2026-07-10-dictation-focus-window-surfaces/dictation-topmost-cycle-1.png`
- `.tmp/visual/2026-07-10-dictation-focus-window-surfaces/dictation-topmost-cycle-2.png`
- `.tmp/visual/2026-07-10-dictation-focus-window-surfaces/runtime-window-state.json`

True exclusive fullscreen can bypass ordinary DWM/HWND overlays and remains outside this desktop-window contract; normal and borderless/fullscreen desktop surfaces passed.

## AC6 — Postmortem and docs

**PASS.** `docs-site/agents/postmortems.md` records symptoms, root cause, fix, regression protection, and prevention guidance.

Commands:

```powershell
rtk bun run docs:sync
rtk bun run docs:check
```

Result: both exit `0`; docs freshness reports no stale references.

## AC7 — Blocked-app overlay transparency and renderer state

**PASS (automated + visual contract).** The fullscreen blocked-app BrowserWindow keeps an alpha background and no longer imports, configures, or applies Electron background material APIs. `FocusBlockOverlay.vue` is unchanged and the regression test confirms its edge-gradient activation, gradient CSS, and popup remain present. A visible renderer/runtime smoke shows the striped application surface through the overlay center while only the intended red edge gradient and blocked-app popup are painted.

Command:

```powershell
rtk test bun test platform/desktop/electron/focus-overlay-window.test.ts
```

Result: `1 pass, 0 fail`. Before the native window fix, the test failed on the existing `backgroundMaterialOption(...)` path. Visual artifacts: `focus-blocked-app-gradient.png` and deterministic `focus-blocked-overlay-example.png`.
