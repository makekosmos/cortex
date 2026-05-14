// Per-extension Vite config — нужен только для dev server'а (HMR).
// Production build всё ещё идёт через корневой vite.extensions.config.mjs.
//
// Запускается через scripts/dev-extensions.mjs:
//   bunx vite --port <devPort> --strictPort --host 127.0.0.1
//
// Порт должен совпадать с manifest.json `devPort`.

import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..", "..");

export default defineConfig({
  plugins: [vue({ features: { vaporInterop: true } }), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src"),
      "@kosmos/ark": path.resolve(repoRoot, "packages/ark/src/index.ts"),
      "@kosmos/visuals/theme/css": path.resolve(
        repoRoot,
        "packages/visuals/theme/css-variables.css",
      ),
      "@kosmos/visuals": path.resolve(repoRoot, "packages/visuals"),
    },
    dedupe: ["vue"],
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    assetsDir: "assets",
    base: "./",
  },
  clearScreen: false,
});
