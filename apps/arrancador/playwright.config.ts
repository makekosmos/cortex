import { defineConfig } from "@playwright/test";

const useExternalServer = process.env.ARRANCADOR_E2E_EXTERNAL_SERVER === "1";
const baseURL = process.env.ARRANCADOR_E2E_BASE_URL ?? "http://127.0.0.1:4174";

export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  expect: { timeout: 5_000 },
  use: {
    baseURL,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
    video: "retain-on-failure",
  },
  ...(useExternalServer
    ? {}
    : {
        webServer: {
          command: "bun run build && bun run preview",
          url: baseURL,
          reuseExistingServer: !process.env.CI,
          timeout: 120_000,
        },
      }),
});
