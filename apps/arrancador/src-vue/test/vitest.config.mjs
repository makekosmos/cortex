import path from "node:path";
import { fileURLToPath } from "node:url";
import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vitest/config";

const rootDir = fileURLToPath(new URL("../../", import.meta.url));

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      "@": path.resolve(rootDir, "./src"),
      "@vue-app": path.resolve(rootDir, "./src-vue"),
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    pool: "threads",
    setupFiles: [path.resolve(rootDir, "./src-vue/test/setup.ts")],
    include: ["src-vue/test/**/*.{test,spec}.{ts,tsx}"],
    exclude: ["e2e/**"],
  },
});
