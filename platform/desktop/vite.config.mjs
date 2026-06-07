import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import electron from "vite-plugin-electron/simple";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [
    vue({ features: { vaporInterop: true } }),
    tailwindcss(),
    electron({
      main: { entry: "electron/main.ts" },
      // Multi-input — главный preload для launcher / settings и shared
      // extension preload для Vue extension'ов (см. electron/extension-host.ts).
      preload: {
        input: {
          preload: "electron/preload.ts",
          "extension-preload": "electron/extension-preload.ts",
        },
        // vite-plugin-electron/simple форсит inlineDynamicImports: true,
        // что несовместимо с multi-input. Перебиваем через nested vite-config.
        // `codeSplitting: true` — новый API (rolldown), подавляет deprecation
        // warning про `inlineDynamicImports`.
        vite: {
          build: {
            rolldownOptions: {
              output: {
                codeSplitting: true,
              },
            },
          },
        },
      },
    }),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@kosmos/ark": path.resolve(__dirname, "../../core/ark/packages/ark/src/index.ts"),
      "@kosmos/visuals/theme/css": path.resolve(
        __dirname,
        "../../packages/visuals/theme/css-variables.css",
      ),
      "@kosmos/visuals": path.resolve(__dirname, "../../packages/visuals"),
      "@raycast/api": path.resolve(__dirname, "../../packages/raycast-api/src/index.ts"),
      "@shared": path.resolve(__dirname, "./shared"),
    },
    dedupe: ["vue"],
  },
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
  },
  clearScreen: false,
});
