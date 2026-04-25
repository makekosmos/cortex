import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import electron from "vite-plugin-electron/simple";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [
    vue({ features: { vaporInterop: true } }),
    electron({
      main: {
        entry: "electron/main.ts",
      },
      preload: {
        input: "electron/preload.ts",
      },
    }),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@kepler/ark": path.resolve(__dirname, "../../packages/kepler-ark/src/index.ts"),
      "@kepler/visuals/theme/css": path.resolve(
        __dirname,
        "../../packages/kepler-visuals/theme/css-variables.css",
      ),
      "@kepler/visuals": path.resolve(__dirname, "../../packages/kepler-visuals"),
      "@shared": path.resolve(__dirname, "./shared"),
    },
    dedupe: ["vue", "vue-router"],
  },
  clearScreen: false,
});
