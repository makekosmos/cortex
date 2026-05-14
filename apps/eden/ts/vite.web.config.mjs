import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [
    vue({ features: { vaporInterop: true } }),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@kepler/visuals": path.resolve(__dirname, "../../../packages/visuals"),
    },
  },
  clearScreen: false,
});
