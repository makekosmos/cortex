// Vitest Browser Mode для Horologion — component-level smoke на реальном
// Chromium. Pattern см. extensions/eden/vitest.config.ts (Phase 6
// bug-detection).
//
// Запуск: `bun run --cwd extensions/horologion test:vue`.

import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
import { playwright } from "@vitest/browser-playwright";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [vue({ features: { vaporInterop: true } })],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@kosmos/ark": path.resolve(__dirname, "../../packages/ark/src/index.ts"),
      "@kosmos/visuals": path.resolve(__dirname, "../../packages/visuals"),
    },
  },
  test: {
    include: ["tests/components/**/*.{test,spec}.ts"],
    browser: {
      enabled: true,
      provider: playwright(),
      headless: true,
      instances: [{ browser: "chromium" }],
    },
  },
});
