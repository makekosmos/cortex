# Verification problems

## AC1 — Triggering the existing Settings action from the current UI opens the Kosmos Settings window again instead of doing nothing.

- **Status:** UNKNOWN
- **Why it is not proven:** This pass only re-ran headless verification. The updated e2e proves the command-bus and renderer IPC entry points in test mode, but no non-headless desktop smoke observed the actual visible Settings window opening from the current UI.
- **Minimal reproduction steps:**
  1. Start the desktop shell in a normal non-headless session.
  2. Trigger the existing Settings action from the launcher/current shell UI.
  3. Observe whether a visible `Kosmos — Настройки` window opens or is restored/focused.
- **Expected vs actual:**
  - Expected: The existing UI action visibly opens or re-focuses the Settings window.
  - Actual: Not verified in this pass; only headless/test-mode behavior was exercised.
- **Affected files:**
  - `platform/desktop/src/views/LauncherView.vue`
  - `platform/desktop/electron/commands.ts`
  - `platform/desktop/electron/settings-window.ts`
- **Smallest safe fix:** Add a non-headless smoke/verification step (manual or reliable headed automation) that triggers the existing UI action and confirms the visible Settings window opens/restores.
- **Corrective hint:** No new code issue is proven here. The remaining gap is user-visible verification of the already-wired UI path outside headless mode.
