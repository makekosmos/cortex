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
      "@kosmos/ark": path.resolve(repoRoot, "core/ark/packages/ark/src/index.ts"),
      "@kosmos/visuals/theme/css": path.resolve(
        repoRoot,
        "packages/visuals/theme/css-variables.css",
      ),
      "@kosmos/visuals": path.resolve(repoRoot, "packages/visuals"),
      // КРИТИЧНО: force vue-router и pinia resolve к extension'овской
      // копии. Bun устанавливает vue-router@4 в packages/visuals/node_modules
      // (peer satisfy) и vue-router@5 в products/delphi/node_modules —
      // разные версии. dedupe не помогает (разные диск-пути). Alias
      // принудительно сводит к одной копии — visuals' SidebarButton.vue
      // RouterLink теперь видит app router instance и рендерит anchor'ы
      // для primaryItems (Inbox/Today) и footerItems (Logbook/Trash).
      "vue-router": path.resolve(__dirname, "node_modules/vue-router"),
      pinia: path.resolve(__dirname, "node_modules/pinia"),
    },
    dedupe: ["vue", "vue-router", "pinia"],
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    assetsDir: "assets",
    base: "./",
  },
  clearScreen: false,
});
