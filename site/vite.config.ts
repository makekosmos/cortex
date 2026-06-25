import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5180,
    hmr: {
      overlay: false,
    },
  },
  resolve: {
    alias: {
      // Те же alias'ы что и platform/desktop/vite.config.mjs — site импортирует
      // реальные desktop-компоненты (LauncherView.vue и т.п.) без копирования.
      "@": path.resolve(__dirname, "../platform/desktop/src"),
      "@shared": path.resolve(__dirname, "../platform/desktop/shared"),
      "@kosmos/visuals/theme/css": path.resolve(
        __dirname,
        "../packages/visuals/theme/css-variables.css",
      ),
      "@kosmos/visuals": path.resolve(__dirname, "../packages/visuals"),
    },
    dedupe: ["vue"],
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
