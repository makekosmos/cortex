import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import electron from "vite-plugin-electron/simple";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [
    vue(),
    tailwindcss(),
    electron({
      main: { entry: "electron/main.ts" },
      preload: { input: "electron/preload.ts" },
    }),
  ],
  resolve: {
    alias: {
      "@kosmos/ark": path.resolve(root, "../../arca-sdk/src/index.ts"),
      "@kosmos/visuals/theme/css": path.resolve(
        root,
        "../../imago/theme/css-variables.css",
      ),
      "@kosmos/visuals": path.resolve(root, "../../imago"),
    },
    dedupe: ["vue"],
  },
  clearScreen: false,
});
