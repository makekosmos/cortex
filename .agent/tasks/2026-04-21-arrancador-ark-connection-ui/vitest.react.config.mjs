import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

const taskDir = fileURLToPath(new URL(".", import.meta.url));
const rootDir = path.resolve(taskDir, "../../../apps/arrancador");

export default defineConfig({
  resolve: {
    alias: {
      "@": path.resolve(rootDir, "./src"),
      "@vue-app": path.resolve(rootDir, "./src-vue"),
      "@kosmos/visuals": path.resolve(rootDir, "../../packages/kosmos-visuals"),
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    pool: "threads",
    setupFiles: [path.resolve(rootDir, "./src-vue/test/setup.ts")],
    include: ["src/test/**/*.{test,spec}.{ts,tsx}"],
    exclude: ["e2e/**"],
  },
});
