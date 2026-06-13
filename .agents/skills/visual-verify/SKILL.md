---
name: visual-verify
description: Verify visible Kosmos UI changes after code edits by launching/reusing a dev target, driving Playwright, capturing screenshots, and inspecting the result. Use for CSS/layout/icon/table/responsive/titlebar/visual polish changes. Do not load during initial diagnosis or for non-visual TS/IPC/data fixes.
---

# Visual Verify

## Workflow

After visible UI changes, verify the result, not only the code:

1. Prefer an existing local dev server if the relevant port is already listening. Do not kill unknown user processes.
2. If no server is running and the app needs one, start the smallest suitable dev server and record the URL.
3. Use Playwright to open the target route. For Electron renderer routes that need preload IPC, inject a minimal `window.kepler` mock with `page.addInitScript`.
4. Wait for the real UI selector that proves the changed surface rendered.
5. Save a screenshot under `.tmp/` and inspect it with `view_image`.
6. Check the actual acceptance criteria visually: contrast, icon rendering, scroll placement, sticky headers, text overflow, spacing, responsive behavior, and active/inactive states.
7. Report what was visually verified and be explicit if any part was not checked.

## Fast Path Artifacts

For `LIGHT_LOOP` UI work, save deterministic screenshots under:

```text
.tmp/visual/<YYYY-MM-DD>-<task-slug>/<surface>-<viewport>-<state>.png
```

Examples:

```text
.tmp/visual/2026-06-03-fast-task-workflow/eden-desktop-empty.png
.tmp/visual/2026-06-03-fast-task-workflow/settings-1000x760-sidebar-filtered.png
```

Final reports should include:

- verified surfaces;
- screenshot artifact paths;
- what was not verified;
- whether escalation or manual follow-up is needed.

Screenshot-only verification is enough only when the change is visual/layout/CSS/text/icon-only, the UI can be rendered with narrow representative mocks or stable local data, and acceptance criteria are visible in static states.

Escalate beyond fast-path screenshot verification when interaction, persistence, IPC, ARK writes, sync, export, usage tracking, native dialogs, tray, UAC, titlebar drag, always-on-top, or real backend behavior is central to the change.

## Playwright Pattern

Use one-off scripts when a permanent test is unnecessary:

```powershell
@'
const { chromium } = require('playwright');
const path = require('node:path');

(async () => {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1000, height: 760 } });

  await page.addInitScript(() => {
    window.kepler = {
      ark: {
        request: async () => [],
      },
    };
  });

  await page.goto('http://localhost:5173/#/dashboard', { waitUntil: 'networkidle' });
  await page.waitForSelector('.dashboard', { timeout: 10000 });
  await page.screenshot({ path: path.resolve('.tmp/visual-check.png'), fullPage: false });
  await browser.close();
})().catch((err) => {
  console.error(err);
  process.exit(1);
});
'@ | node -
```

Keep mocks narrow and representative. If the UI depends on real data shape, include enough mocked rows to exercise the states the user cares about.
