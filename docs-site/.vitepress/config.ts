import { defineConfig } from "vitepress";
import { themeConfig } from "./theme-config";

export default defineConfig({
  title: "Kosmos",
  titleTemplate: ":title · Kosmos",
  description: "Документация Kosmos — local-first монорепо с ARK runtime и Electron-приложениями.",
  lang: "ru",
  cleanUrls: true,
  lastUpdated: true,
  appearance: "dark",
  head: [
    ["meta", { name: "color-scheme", content: "dark light" }],
    ["meta", { name: "theme-color", content: "#1c1c1c" }],
  ],
  // Убираем modulepreload для тяжёлых ленивых чанков. Они будут загружены
  // динамическим import() только когда реально нужны.
  transformHtml(code) {
    return code.replace(
      /\s*<link rel="modulepreload" href="[^"]*\/(?:mermaid|wardley|pan-zoom|cytoscape|elkjs|d3-)[^"]*"\s*\/?>/g,
      "",
    );
  },

  markdown: {
    lineNumbers: false,
    theme: { light: "github-light", dark: "github-dark" },
  },

  // Игнорируем ссылки на артефакты вне docs-site/ — .agent/.agents/
  // указывают на корневые директории репо (skills, proof loops), которые
  // не публикуются в дока-сайт, но полезны при чтении исходников локально.
  ignoreDeadLinks: [/^\/\.agents?\//],

  vite: {
    build: {
      chunkSizeWarningLimit: 3000,
      // Отключаем modulepreload полностью — иначе VitePress препрефетчит
      // тяжёлый mermaid chunk (2.8MB) на каждой странице, даже без диаграмм.
      // Цена: при навигации chunk страницы качается чуть позже (~50-100ms).
      modulePreload: false,
      rollupOptions: {
        output: {
          manualChunks(id) {
            if (id.includes("node_modules/mermaid")) return "mermaid";
            if (id.includes("node_modules/svg-pan-zoom")) return "pan-zoom";
            if (id.includes("node_modules/cytoscape")) return "mermaid";
            if (id.includes("node_modules/dagre")) return "mermaid";
            if (id.includes("node_modules/elkjs")) return "mermaid";
            if (id.match(/node_modules\/d3-/)) return "mermaid";
          },
        },
      },
    },
    // Mermaid (v11) и его внутренние динамические импорты ломаются в dev
    // когда вынесены в exclude — Vite не пре-бандлит, и relative chunks падают.
    // В prod modulePreload: false + manualChunks: "mermaid" всё равно держит
    // 2.8MB chunk ленивым (load на демонд из нашего theme/index.ts).
    optimizeDeps: {
      include: ["mermaid", "svg-pan-zoom"],
    },
  },

  themeConfig,
});
