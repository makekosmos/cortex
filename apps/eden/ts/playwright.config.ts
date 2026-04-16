import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  testIgnore: ["tests/typing-stress.spec.ts"],
  timeout: 30000,
  expect: {
    timeout: 5000,
  },
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  // Electron window bootstrap is flaky when multiple app instances launch in parallel.
  workers: 1,
  reporter: "html",
  use: {
    actionTimeout: 0,
    trace: "on-first-retry",
  },
});
