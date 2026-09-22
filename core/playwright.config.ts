import { defineConfig } from "@playwright/test";

// Playwright config для Kepler shell + extensions e2e.
//
// Все specs живут в tests/e2e/. Каждый spec спавнит Electron через
// `_electron.launch(...)` с обязательным `KOSMOS_DATA_DIR` env override —
// никогда не указывай тестам real user data dir (`%APPDATA%\Kosmos\`).
//
// Browsers (chromium/firefox/webkit) не нужны: тестируем Electron app
// напрямую, не web. Поэтому `bunx playwright install` не обязателен.

export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 30_000,
  expect: {
    timeout: 5_000,
  },
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  reporter: process.env.CI ? "github" : "list",
  use: {
    actionTimeout: 10_000,
    trace: "retain-on-failure",
  },
});
