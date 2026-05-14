import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@playwright/test";

const appRoot = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  expect: { timeout: 5_000 },
  fullyParallel: false,
  workers: 1,
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: [["list"]],
  outputDir: "test-results",
  use: {
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  metadata: {
    appRoot,
    // Изолированный userData — чтобы не конфликтовать с user-installed Kepler.
    // См. docs-site/concepts/test-isolation.md.
    userDataDir: path.join(appRoot, ".e2e", "kepler-shell-userdata"),
  },
});
