// Build config для Kepler Vue extension bundles.
//
// Source packages are discovered from products/*, incubator/*, and the
// deprecated compatibility root extensions/*.
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
import { existsSync } from "node:fs";
import {
  findRepoExtensionEntry,
  listRepoExtensionEntries,
} from "./scripts/repo-extension-roots.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..", "..");

function discoverVueExtensions() {
  return listRepoExtensionEntries(repoRoot)
    .filter((entry) => {
      const mainTs = path.join(entry.dir, "src", "main.ts");
      const indexHtml = path.join(entry.dir, "index.html");
      return existsSync(mainTs) && existsSync(indexHtml);
    })
    .map((entry) => entry.id);
}

export const vueExtensions = discoverVueExtensions();

export default defineConfig(({ mode }) => {
  // mode используется как extension id. По умолчанию — первый
  // найденный extension, что удобно для smoke `vite build --config ...`.
  const id = mode && mode !== "production" && mode !== "development" ? mode : vueExtensions[0];
  const entry = id ? findRepoExtensionEntry(repoRoot, id) : null;

  if (!entry) {
    throw new Error(
      "[vite.extensions] no Vue extensions discovered (need src/main.ts + index.html)",
    );
  }

  const extensionDir = entry.dir;
  if (!existsSync(path.join(extensionDir, "index.html"))) {
    throw new Error(`[vite.extensions] extension '${entry.id}' has no index.html`);
  }

  return {
    root: extensionDir,
    base: "./",
    // `@tailwindcss/vite` v4 is a no-op for CSS files that don't `@import "tailwindcss"`,
    // so включение plugin'а глобально безопасно для extensions без Tailwind
    // (dashboard). Активируется только для delphi/arrancador,
    // у которых tailwind directives есть в их CSS entry.
    plugins: [vue({ features: { vaporInterop: true } }), tailwindcss()],
    resolve: {
      alias: {
        // Per-extension "@" alias → <extensionDir>/src. Совпадает с конвенцией
        // legacy apps (Delphi, Eden), упрощает миграцию исходников
        // как Vue extension без массового rewrite import-путей.
        "@": path.resolve(extensionDir, "src"),
        "@kosmos/ark": path.resolve(__dirname, "../../core/ark/packages/ark/src/index.ts"),
        "@kosmos/visuals/theme/css": path.resolve(
          __dirname,
          "../../packages/visuals/theme/css-variables.css",
        ),
        "@kosmos/visuals": path.resolve(__dirname, "../../packages/visuals"),
        // Extensions live at <repoRoot>/extensions/<id>/ and have no own
        // node_modules. Bare-specifier deps used by extension sources
        // (e.g. @lucide/vue, @phosphor-icons/vue, tailwindcss) resolve via shell's
        // node_modules: point them explicitly so Rolldown does not walk
        // up past the repo root and miss them.
        "@lucide/vue": path.resolve(__dirname, "node_modules/@lucide/vue"),
        "@phosphor-icons/vue": path.resolve(__dirname, "node_modules/@phosphor-icons/vue"),
        // Tailwind CSS — extension'ы могут @import "tailwindcss" (или
        // его submodules как `tailwindcss/utilities.css`). Bare specifier
        // не резолвится из <extensionDir>/src без alias'а, потому что
        // extension'ы не имеют локального node_modules.
        tailwindcss: path.resolve(__dirname, "node_modules/tailwindcss"),
        // Force vue-router/pinia resolve к extension'овской копии
        // (extensions/<id>/node_modules/). Bun pinning creates separate
        // copies in packages/visuals/node_modules (peer satisfy) → разные
        // RouterLink injection symbols → primary/footer sidebar items не
        // рендерятся. Alias привязывает к одной копии. Только для extension'ов
        // у которых эта зависимость реально установлена.
        ...(existsSync(path.join(extensionDir, "node_modules/vue-router"))
          ? { "vue-router": path.resolve(extensionDir, "node_modules/vue-router") }
          : {}),
        ...(existsSync(path.join(extensionDir, "node_modules/pinia"))
          ? { pinia: path.resolve(extensionDir, "node_modules/pinia") }
          : {}),
      },
      dedupe: ["vue", "vue-router", "pinia"],
    },
    build: {
      outDir: path.join(extensionDir, "dist"),
      emptyOutDir: true,
      // Asset path-ы relative к dist/index.html (base: "./") — нужны для
      // file:// загрузки через BrowserWindow.loadFile().
      assetsDir: "assets",
      // Eden Editor lazy-chunk весит ~1.3MB (TipTap + lowlight + grammars) —
      // он async-loaded, в main bundle Eden остаётся ~360KB. Дефолтный
      // warning порог 500KB здесь бесполезен.
      chunkSizeWarningLimit: 1500,
    },
    clearScreen: false,
  };
});
