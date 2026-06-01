---
name: visual-verify
description: Verify Kosmos UI changes by launching or reusing a local dev target, driving it with Playwright, capturing screenshots, and visually inspecting the result. Use after frontend, Vue, CSS, icon, layout, table, responsive, titlebar, or visual polish changes where typecheck/build alone cannot prove the UI looks correct.
---

# Visual Verify

## Workflow

After UI changes, verify the visible result, not only the code:

1. Prefer an existing local dev server if the relevant port is already listening. Do not kill unknown user processes.
2. If no server is running and the app needs one, start the smallest suitable dev server and record the URL.
3. Use Playwright to open the target route. For Electron renderer routes that need preload IPC, inject a minimal `window.kepler` mock with `page.addInitScript`.
4. Wait for the real UI selector that proves the changed surface rendered.
5. Save a screenshot under `.tmp/` and inspect it with `view_image`.
6. Check the actual acceptance criteria visually: contrast, icon rendering, scroll placement, sticky headers, text overflow, spacing, responsive behavior, and active/inactive states.
7. Report what was visually verified and be explicit if any part was not checked.

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
