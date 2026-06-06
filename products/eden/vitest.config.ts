// Vitest Browser Mode для Eden — component-level тесты на реальном Chromium.
// Зачем: jsdom не воспроизводит Vue 3.6 Vapor edge-cases и реальные браузерные
// API (ResizeObserver, IntersectionObserver, contenteditable, focus events).
// Real browser ловит регрессии которые иначе всплыли бы только в e2e.
//
// Запуск: `bun run test:vue` (см. package.json scripts).
//
// Layer cake тестов Eden:
//   - bun:test (tests/*.test.ts) — pure JS unit (charCount, lib/*).
//   - vitest browser (tests/components/*.spec.ts) — Vue component на Chromium.
//   - Playwright e2e (root tests/e2e/eden.spec.ts) — full Electron flow.

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
      "@kosmos/visuals": path.resolve(__dirname, "../../packages/visuals"),
    },
  },
  optimizeDeps: {
    include: ["@tiptap/pm/state"],
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
