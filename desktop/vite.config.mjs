import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import electron from "vite-plugin-electron/simple";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { resolveWorkspacePaths } from "../scripts/workspace.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const workspacePaths = resolveWorkspacePaths(path.resolve(__dirname, ".."), process.env);

export default defineConfig({
  plugins: [
    vue({ features: { vaporInterop: true } }),
    tailwindcss(),
    electron({
      main: { entry: "electron/main.ts" },
      // Главный preload для launcher / settings и shared host.
      preload: {
        input: "electron/preload.ts",
      },
    }),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@kosmos/ark": path.join(workspacePaths["arca-sdk"], "src/index.ts"),
      "@kosmos/visuals/theme/css": path.resolve(
        __dirname,
        path.join(workspacePaths.imago, "theme/css-variables.css"),
      ),
      "@kosmos/visuals": workspacePaths.imago,
      "@raycast/api": path.resolve(__dirname, "../packages/raycast-api/src/index.ts"),
      "@shared": path.resolve(__dirname, "./shared"),
    },
    dedupe: ["vue"],
  },
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    hmr: {
      overlay: false,
    },
  },
  clearScreen: false,
});
