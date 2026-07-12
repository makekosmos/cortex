import { defineConfig } from "../../platform/desktop/node_modules/vite/dist/node/index.js";
import vue from "../../platform/desktop/node_modules/@vitejs/plugin-vue/dist/index.mjs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..", "..");

export default defineConfig({
  base: "./",
  cacheDir: path.resolve(repoRoot, ".tmp/vite-cache/daedalus"),
  plugins: [vue()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src"),
      "@kosmos/ark/agents": path.resolve(
        repoRoot,
        "core/ark/packages/ark/src/ark-client-agents.ts",
      ),
      "@kosmos/ark": path.resolve(repoRoot, "core/ark/packages/ark/src/index.ts"),
      "@kosmos/visuals/theme/css": path.resolve(
        repoRoot,
        "packages/visuals/theme/css-variables.css",
      ),
      "@kosmos/visuals": path.resolve(repoRoot, "packages/visuals"),
      vue: path.resolve(__dirname, "node_modules/vue"),
      pinia: path.resolve(__dirname, "node_modules/pinia"),
    },
    dedupe: ["vue", "pinia"],
  },
  build: { outDir: "dist", emptyOutDir: true, assetsDir: "assets" },
  server: { hmr: { overlay: false } },
  test: { environment: "jsdom", include: ["tests/**/*.test.ts"] },
  clearScreen: false,
});
