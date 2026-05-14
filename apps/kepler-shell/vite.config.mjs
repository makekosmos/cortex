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
      main: { entry: "electron/main.ts" },
      preload: { input: "electron/preload.ts" },
    }),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@kosmos/ark": path.resolve(__dirname, "../../packages/kosmos-ark/src/index.ts"),
      "@kosmos/visuals/theme/css": path.resolve(
        __dirname,
        "../../packages/kosmos-visuals/theme/css-variables.css",
      ),
      "@kosmos/visuals": path.resolve(__dirname, "../../packages/kosmos-visuals"),
      "@shared": path.resolve(__dirname, "./shared"),
    },
    dedupe: ["vue"],
  },
  clearScreen: false,
});
