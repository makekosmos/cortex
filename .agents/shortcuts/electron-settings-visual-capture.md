# Electron Settings Visual Capture

## Trigger

Use when verifying Kosmos settings UI in Electron test/headless mode and normal
Playwright screenshots time out, capture a stale frame, or seem to show a
different settings tab than the DOM assertions.

## Symptom

`page.screenshot()` or `locator.screenshot()` times out, or an Electron
`capturePage()` PNG shows an old/default tab even though Playwright locators
find the expected content.

## Do This

1. Launch the same built Electron target used by e2e:
   `platform/desktop/dist-electron/main.js`.
2. Open settings through `window.kepler.commands.invoke("settings:open")`.
3. Click the target settings tab and assert a unique text from that tab with
   Playwright locators.
4. In `app.evaluate`, select `BrowserWindow` by
   `win.webContents.getURL().includes("#settings")`.
5. Verify `document.body.innerText` in that exact window contains the expected
   tab text before saving a PNG.
6. For advanced settings pages, scroll `.advanced-page`, not
   `.settings-content`, before taking a lower-page capture.

## Avoid

- Do not trust a PNG until the same `BrowserWindow` body text confirms the
  expected tab content.
- Do not scroll `.settings-content` for advanced pages; it is a flex parent,
  while `.advanced-page` owns the scroll.

## Promote To Skill When

Promote if more Electron visual checks need reusable scripts or this pattern
appears outside settings windows.
