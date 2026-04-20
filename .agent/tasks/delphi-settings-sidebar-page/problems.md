# Problems Found After Initial Verification

## P1: Settings shell did not replace the app sidebar

- Reported by user after implementation review.
- `SettingsPage.vue` rendered its own internal sidebar, but `apps/delphi/ts/src/App.vue` still mounted the main Delphi sidebar in the `DesktopChrome` sidebar slot for `/settings`.
- Result: Delphi showed the normal app sidebar on the far left and the settings sidebar inside the content area, unlike `eden/ts` where settings replace the whole left sidebar region.

## Smallest Safe Fix

- Make `App.vue` switch sidebar chrome off for `route.path === "/settings"`.
- Reuse the same gating for `DesktopContentSurface` left border and top-left radius so the settings page owns the full left edge cleanly.
