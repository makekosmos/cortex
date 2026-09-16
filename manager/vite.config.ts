import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import electron from "vite-plugin-electron/simple";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { resolveWorkspacePaths } from "../scripts/workspace.mjs";

const root = path.dirname(fileURLToPath(import.meta.url));
const workspacePaths = resolveWorkspacePaths(path.resolve(root, ".."), process.env);
const externalElectron = process.env.KOSMOS_MANAGER_EXTERNAL_ELECTRON === "1";

export default defineConfig({
  server: {
    fs: {
      allow: [path.resolve(root, "../..")],
    },
    watch: {
      // dev-run keeps engine data under .dev/; a recursive watcher would hold
      // a directory handle on package staging dirs and break install renames.
      ignored: ["**/.dev/**"],
    },
  },
  plugins: [
    vue(),
    tailwindcss(),
    ...(!externalElectron
      ? [
          electron({
            main: { entry: "electron/main.ts" },
            preload: { input: "electron/preload.ts" },
          }),
        ]
      : []),
  ],
  resolve: {
    alias: {
      "@kosmos/ark": path.join(workspacePaths["arca-sdk"], "src/index.ts"),
      "@kosmos/visuals/theme/css": path.resolve(
        root,
        path.join(workspacePaths.imago, "theme/css-variables.css"),
      ),
      "@kosmos/visuals/css": path.join(workspacePaths.imago, "dist/index.css"),
      "@kosmos/visuals": path.join(workspacePaths.imago, "index.ts"),
    },
    dedupe: ["vue"],
  },
  clearScreen: false,
});
