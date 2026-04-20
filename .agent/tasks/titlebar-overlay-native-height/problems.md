# Problems

## Adaptive runtime height approach

The previous renderer-measured overlay height approach did not produce the desired native caption-button behavior in practice.

Observed issue:
- custom `titleBarOverlay.height` synchronization added complexity and still did not make the controls behave correctly

Smallest safe fix:
- remove renderer-side height measurement and IPC
- remove custom `titleBarOverlay.height`
- let Electron/Windows use the standard native overlay height automatically
