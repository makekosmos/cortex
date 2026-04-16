import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  testMatch: ["typing-stress.spec.ts"],
  timeout: 120000,
  fullyParallel: false,
  workers: 1,
  reporter: "html",
  use: {
    actionTimeout: 0,
    trace: "on-first-retry",
  },
});
