import { defineConfig } from "vite";
import electron from "vite-plugin-electron/simple";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(fileURLToPath(import.meta.url));
export default defineConfig({
  plugins: [
    electron({ main: { entry: "electron/main.ts" }, preload: { input: "electron/preload.ts" } }),
  ],
  resolve: {
    alias: { "@kosmos/ark": path.resolve(root, "../../arca-sdk/src/index.ts") },
  },
  build: { outDir: "dist" },
});
