# Visual Verify With Edge

## Trigger

Use this shortcut when a Kosmos UI visual check needs Playwright on Windows and bundled Chromium is missing or not installed.

## Symptom

`chromium.launch()` fails before the page opens because Playwright cannot find its downloaded browser, even though Microsoft Edge is installed.

## Do This

Launch Playwright with the system Edge channel:

```js
const browser = await chromium.launch({ headless: true, channel: "msedge" });
```

Keep the rest of the visual verification flow unchanged: inject narrow `window.kepler` / `window.api` mocks, wait for the real UI selector, save screenshots under `.tmp/visual/<date>-<slug>/`, then inspect them.

## Avoid

Do not reinstall browsers or kill dev servers just to capture a screenshot if the system Edge channel works.

## Promote To Skill When

Promote this into `visual-verify` if Playwright browser-install failures become common across Kosmos UI tasks.
