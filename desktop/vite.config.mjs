import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import electron from "vite-plugin-electron/simple";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { resolveWorkspacePaths } from "../scripts/workspace.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const workspacePaths = resolveWorkspacePaths(path.resolve(__dirname, ".."), process.env);
const electronMain = { entry: "electron/main.ts" };
if (process.env.KOSMOS_DEV_RUN_MANAGED === "1") electronMain.onstart = () => undefined;

export default defineConfig({
  plugins: [
    vue({ features: { vaporInterop: true } }),
    tailwindcss(),
    electron({
      main: electronMain,
      // Главный preload для settings / overlay окон и shared host.
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
      // exports map: ./components -> ./packages/vue/src/components
      "@kosmos/visuals/components": path.join(workspacePaths.imago, "packages/vue/src/components"),
      "@kosmos/visuals": workspacePaths.imago,
      "@shared": path.resolve(__dirname, "./shared"),
    },
    dedupe: ["vue"],
  },
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    fs: {
      allow: [workspacePaths.imago, workspacePaths["arca-sdk"]],
    },
    hmr: {
      overlay: false,
    },
    watch: {
      // dev-run keeps engine data under .e2e/; a recursive watcher would hold
      // a directory handle on package staging dirs and break install renames.
      ignored: ["**/.e2e/**"],
    },
  },
  clearScreen: false,
});
