// Per-extension Vite config — нужен только для dev server'а (HMR).
// Production build идёт через корневой platform/desktop/vite.extensions.config.mjs.
//
// Запускается через platform/desktop/scripts/dev-extensions.mjs:
//   bunx vite --port 5184 --strictPort --host 127.0.0.1
//
// Порт совпадает с manifest.json `devPort`.

import { defineConfig } from "../../platform/desktop/node_modules/vite/dist/node/index.js";
import vue from "../../platform/desktop/node_modules/@vitejs/plugin-vue/dist/index.mjs";
import tailwindcss from "../../platform/desktop/node_modules/@tailwindcss/vite/dist/index.mjs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..", "..");

export default defineConfig({
  cacheDir: path.resolve(repoRoot, ".tmp/vite-cache/eden"),
  plugins: [vue({ features: { vaporInterop: true } }), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src"),
      tailwindcss: path.resolve(repoRoot, "platform/desktop/node_modules/tailwindcss"),
      "@kosmos/visuals/theme/css": path.resolve(
        repoRoot,
        "packages/visuals/theme/css-variables.css",
      ),
      "@kosmos/visuals": path.resolve(repoRoot, "packages/visuals"),
      pinia: path.resolve(__dirname, "node_modules/pinia"),
    },
    dedupe: ["vue", "pinia"],
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    assetsDir: "assets",
    base: "./",
  },
  clearScreen: false,
});
