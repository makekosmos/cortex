# Dictation and Focus window surfaces

## Classification

`FULL_LOOP`: the change touches two shell-owned native Electron windows, including Focus mode safety/window behavior, and requires runtime/visual proof beyond a local CSS edit.

## Goal

Restore the intended Windows behavior for the dictation pill and Focus widget:

- the dictation pill remains above other application windows while active;
- the Focus widget has a genuinely transparent native window background, with no rectangular white, black, or opaque backing area around its rounded UI.
- the fullscreen Focus blocked-app overlay preserves its intentional edge gradient and popup while the rest of the native window remains transparent.

## Scope amendment — 2026-07-10

After the initial spec was frozen, the user explicitly clarified that “Focus” includes the blocked-app state with the gradient around the screen edges. The fullscreen `focus-overlay` surface is therefore added to this proof loop; no other Focus behavior is expanded.

## In scope

- `platform/desktop/electron/` creation and runtime behavior for the dictation pill and Focus widget windows;
- `platform/desktop/electron/focus-overlay.ts` native backing behavior and preservation of the existing `FocusBlockOverlay.vue` gradient/popup presentation;
- renderer/global styling only if evidence shows it contributes to the Focus backing surface;
- focused regression coverage for the native window contracts;
- runtime or visual evidence on Windows where locally possible.

## Out of scope

- dictation recording/transcription behavior;
- Pomodoro lifecycle, persistence, hosts-file blocking, or ARK data paths;
- redesigning either widget;
- release, version bump, commit, or push.

## Acceptance criteria

**AC1.** While the dictation pill is active, its Electron window is configured and maintained at the Windows always-on-top level used for overlay surfaces, including after show/reactivation; focused regression coverage prevents the window from silently returning to an ordinary z-order.

**AC2.** The Focus widget's native Electron window uses a transparent backing surface and does not request an opaque/background material that paints the rectangular BrowserWindow area behind the rounded UI.

**AC3.** The Focus widget keeps its existing interaction and lifecycle behavior: it remains shell-owned, uses existing `kepler:focus-widget:*` IPC, and no Pomodoro/backend or hosts-file boundary is changed.

**AC4.** Focused automated checks for the touched desktop window contracts pass on the final working tree.

**AC5.** A Windows runtime/visual check demonstrates the dictation pill z-order and Focus transparency, or the evidence explicitly records the exact environmental limitation and the strongest available static/automated substitute.

**AC6.** The recurring native-window regression is documented with root cause, fix, regression protection, and prevention guidance in `docs-site/agents/postmortems.md`, and documentation checks pass.

**AC7.** The fullscreen blocked-app Focus overlay does not request or apply an Electron background material; its native surface stays transparent while the renderer retains the existing edge-gradient and popup state.
