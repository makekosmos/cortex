import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@playwright/test";

const managerRoot = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  testDir: path.join(managerRoot, "e2e"),
  globalSetup: path.join(managerRoot, "e2e", "global-setup.ts"),
  timeout: 90_000,
  expect: { timeout: 10_000 },
  fullyParallel: false,
  workers: 1,
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: [["list"]],
  use: { trace: "retain-on-failure" },
});
