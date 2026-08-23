// Build config для Kepler Vue extension bundles.
//
// Source packages are discovered from products/* and the deprecated
// compatibility root extensions/*.
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
    // (dashboard). Активируется только для Delphi, у которого tailwind
    // directives есть в CSS entry.
    plugins: [vue({ features: { vaporInterop: true } }), tailwindcss()],
    resolve: {
      alias: [
        // Vite 8/Rolldown не резолвит extensionless highlight.js grammar subpaths из lowlight.
        // См. docs-site/agents/postmortems.md § 2026-06-13.
        {
          find: /^highlight\.js\/lib\/languages\/(.+)$/,
          replacement: path.resolve(
            repoRoot,
            "node_modules/.bun/highlight.js@11.11.1/node_modules/highlight.js/es/languages/$1.js",
          ),
        },
        {
          find: /^highlight\.js\/lib\/(.+)$/,
          replacement: path.resolve(
            repoRoot,
            "node_modules/.bun/highlight.js@11.11.1/node_modules/highlight.js/es/$1.js",
          ),
        },
        {
          find: /^highlight\.js$/,
          replacement: path.resolve(
            repoRoot,
            "node_modules/.bun/highlight.js@11.11.1/node_modules/highlight.js/es/index.js",
          ),
        },
        {
          find: /^@\/(.+)$/,
          replacement: path.resolve(extensionDir, "src/$1"),
        },
        {
          find: /^@$/,
          replacement: path.resolve(extensionDir, "src"),
        },
        {
          find: /^@kosmos\/ark$/,
          replacement: path.resolve(__dirname, "../../arca-sdk/src/index.ts"),
        },
        {
          find: /^@kosmos\/visuals\/theme\/css$/,
          replacement: path.resolve(__dirname, "../../imago/theme/css-variables.css"),
        },
        {
          find: /^@kosmos\/visuals\/(.+)$/,
          replacement: path.resolve(__dirname, "../../imago/$1"),
        },
        {
          find: /^@kosmos\/visuals$/,
          replacement: path.resolve(__dirname, "../../imago"),
        },
        {
          find: /^@lucide\/vue$/,
          replacement: path.resolve(__dirname, "node_modules/@lucide/vue"),
        },
        {
          find: /^@phosphor-icons\/vue$/,
          replacement: path.resolve(__dirname, "node_modules/@phosphor-icons/vue"),
        },
        {
          find: /^tailwindcss\/(.+)$/,
          replacement: path.resolve(__dirname, "node_modules/tailwindcss/$1"),
        },
        {
          find: /^tailwindcss$/,
          replacement: path.resolve(__dirname, "node_modules/tailwindcss"),
        },
        ...(existsSync(path.join(extensionDir, "node_modules/vue-router"))
          ? [
              {
                find: /^vue-router$/,
                replacement: path.resolve(extensionDir, "node_modules/vue-router"),
              },
            ]
          : []),
        ...(existsSync(path.join(extensionDir, "node_modules/pinia"))
          ? [{ find: /^pinia$/, replacement: path.resolve(extensionDir, "node_modules/pinia") }]
          : []),
      ],
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
