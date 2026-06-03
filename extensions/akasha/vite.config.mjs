import vue from "../../shell/node_modules/@vitejs/plugin-vue/dist/index.mjs";
import tailwindcss from "../../shell/node_modules/@tailwindcss/vite/dist/index.mjs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..", "..");

export default {
  root: __dirname,
  plugins: [vue({ features: { vaporInterop: true } }), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src"),
      "@kosmos/visuals/theme/css": path.resolve(
        repoRoot,
        "packages/visuals/theme/css-variables.css",
      ),
      "@kosmos/visuals": path.resolve(repoRoot, "packages/visuals"),
      "@lucide/vue": path.resolve(repoRoot, "shell/node_modules/@lucide/vue"),
      "@phosphor-icons/vue": path.resolve(repoRoot, "shell/node_modules/@phosphor-icons/vue"),
      tailwindcss: path.resolve(repoRoot, "shell/node_modules/tailwindcss"),
    },
    dedupe: ["vue"],
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    assetsDir: "assets",
    base: "./",
  },
  clearScreen: false,
};
