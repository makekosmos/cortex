# Windows Transparent Overlay Visual Proof

## Trigger

Use when a transparent Electron `BrowserWindow` or always-on-top overlay must be visually verified on Windows.

## Symptom

`CopyFromScreen` or `desktopCapturer` screenshots contain large black or stale rectangles while a hardware-accelerated fullscreen app changes foreground state, making a working transparent overlay look opaque.

## Do This

1. Keep DevTools closed and verify the native window contract (`transparent: true`, alpha `backgroundColor`, no DWM material API).
2. Run the overlay against a separate high-contrast fullscreen process and capture the real OS screen for native DWM/z-order evidence.
3. Enumerate visible HWND bounds before blaming the overlay; hide only test-owned windows that accidentally cover the probe surface.
4. Save a deterministic renderer screenshot on a contrast background for a clean state example, but pair it with the native contract/runtime check.
5. For reused topmost windows, force the background app to the foreground between hide/show cycles and confirm the overlay remains visible.

## Avoid

- Treating a black GPU capture region as proof that the user sees an opaque window.
- Using a renderer screenshot alone to claim native DWM transparency or z-order behavior.
- Leaving a test launcher window visible behind a fullscreen transparent overlay.

## Promote To Skill When

Promote when this workflow is needed across several native overlay families or requires a maintained helper script.
