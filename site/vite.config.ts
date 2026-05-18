import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [vue()],
  server: { port: 5180 },
  resolve: {
    alias: {
      // Те же alias'ы что и shell/vite.config.mjs — site импортирует
      // реальные shell-компоненты (LauncherView.vue и т.п.) без копирования.
      "@": path.resolve(__dirname, "../shell/src"),
      "@shared": path.resolve(__dirname, "../shell/shared"),
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
