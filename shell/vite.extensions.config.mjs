// Build config для Kepler Vue extension bundles.
//
// Каждая поддиректория `extensions/<id>/` с `src/main.ts` и `index.html`
// считается build target'ом и собирается в `extensions/<id>/dist/`.
//
// Запуск: vite build --config vite.extensions.config.mjs --mode <id>
// (см. scripts.build:extensions в package.json — orchestrator проходит по
// всем extension id и запускает build на каждом).
//
// Static-extensions (PoC без src/main.ts) этим конфигом игнорируются — они
// используются как есть (manifest.entryHtml: "index.html").

import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { existsSync, readdirSync, statSync } from "node:fs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const extensionsRoot = path.resolve(__dirname, "..", "extensions");

function discoverVueExtensions() {
  if (!existsSync(extensionsRoot)) return [];
  return readdirSync(extensionsRoot, { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => d.name)
    .filter((id) => {
      const mainTs = path.join(extensionsRoot, id, "src", "main.ts");
      const indexHtml = path.join(extensionsRoot, id, "index.html");
      try {
        return statSync(mainTs).isFile() && statSync(indexHtml).isFile();
      } catch {
        return false;
      }
    });
}

export const vueExtensions = discoverVueExtensions();

export default defineConfig(({ mode }) => {
  // mode используется как extension id. По умолчанию — первый
  // найденный extension, что удобно для smoke `vite build --config ...`.
  const id = mode && mode !== "production" && mode !== "development"
    ? mode
    : vueExtensions[0];

  if (!id) {
    throw new Error(
      "[vite.extensions] no Vue extensions discovered (need src/main.ts + index.html)",
    );
  }

  const extensionDir = path.join(extensionsRoot, id);
  if (!existsSync(path.join(extensionDir, "index.html"))) {
    throw new Error(`[vite.extensions] extension '${id}' has no index.html`);
  }

  return {
    root: extensionDir,
    base: "./",
    // `@tailwindcss/vite` v4 is a no-op for CSS files that don't `@import "tailwindcss"`,
    // so включение plugin'а глобально безопасно для extensions без Tailwind
    // (dashboard, horologion). Активируется только для delphi/arrancador,
    // у которых tailwind directives есть в их CSS entry.
    plugins: [vue({ features: { vaporInterop: true } }), tailwindcss()],
    resolve: {
      alias: {
        // Per-extension "@" alias → <extensionDir>/src. Совпадает с конвенцией
        // legacy apps (Delphi, Eden, Horologion), упрощает миграцию исходников
        // как Vue extension без массового rewrite import-путей.
        "@": path.resolve(extensionDir, "src"),
        "@kosmos/ark": path.resolve(
          __dirname,
          "../packages/ark/src/index.ts",
        ),
        "@kosmos/visuals/theme/css": path.resolve(
          __dirname,
          "../packages/visuals/theme/css-variables.css",
        ),
        "@kosmos/visuals": path.resolve(
          __dirname,
          "../packages/visuals",
        ),
        // Extensions live at <repoRoot>/extensions/<id>/ and have no own
        // node_modules. Bare-specifier deps used by extension sources
        // (e.g. lucide-vue-next, tailwindcss) resolve via shell's
        // node_modules: point them explicitly so Rolldown does not walk
        // up past the repo root and miss them.
        "lucide-vue-next": path.resolve(
          __dirname,
          "node_modules/lucide-vue-next",
        ),
      },
      dedupe: ["vue"],
    },
    build: {
      outDir: path.join(extensionDir, "dist"),
      emptyOutDir: true,
      // Asset path-ы relative к dist/index.html (base: "./") — нужны для
      // file:// загрузки через BrowserWindow.loadFile().
      assetsDir: "assets",
    },
    clearScreen: false,
  };
});
