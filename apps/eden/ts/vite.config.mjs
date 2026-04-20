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
        entry: "main/main.ts",
      },
      preload: {
        input: "main/preload.ts",
      },
    }),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@kepler/visuals": path.resolve(__dirname, "../../../packages/kepler-visuals"),
    },
  },
  clearScreen: false,
});
