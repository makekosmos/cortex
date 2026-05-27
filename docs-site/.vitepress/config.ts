import { defineConfig } from "vitepress";

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

  themeConfig: {
    siteTitle: "Kosmos",

    // Порядок зафиксирован — на нём держится conditional CSS в custom.css
    // (body[data-kosmos-mode="user"|"dev"]). См. Layout.vue / ManualSidebarHeader.vue.
    //   1. Мануал           — всегда
    //   2. Новости          — всегда
    //   3. Для разработчиков — только в user mode (manual/news)
    //   4-9. Dev nav        — только в dev mode
    nav: [
      { text: "Мануал", link: "/manual/" },
      { text: "Новости", link: "/whats-new/" },
      { text: "Для разработчиков", link: "/guide/getting-started" },
      { text: "Старт", link: "/guide/getting-started" },
      { text: "Концепты", link: "/concepts/architecture" },
      { text: "Приложения", link: "/apps/" },
      { text: "Пакеты", link: "/packages/" },
      { text: "Эксперименты", link: "/experiments/" },
      { text: "Справочник", link: "/reference/rules" },
    ],

    sidebar: {
      // /manual/* — config нужен чтобы VitePress отрисовал sidebar-колонку
      // и search-index видел страницы. Сам визуальный sidebar полностью
      // переопределяется кастомным компонентом ManualSidebarHeader.vue
      // (через slot 'sidebar-nav-before'), а дефолтные .VPSidebarItem'ы
      // прячутся CSS-ом на /manual/* (см. custom.css).
      "/manual/": [
        {
          text: "Начало",
          link: "/manual/basics",
          items: [
            { text: "Быстрый старт", link: "/manual/basics" },
            { text: "Платформы", link: "/manual/platforms" },
          ],
        },
        {
          text: "Core features",
          link: "/manual/core-features/",
          items: [
            { text: "Обзор", link: "/manual/core-features/" },
            { text: "Запуск приложений", link: "/manual/core-features/app-launcher" },
          ],
        },
        {
          text: "Встроенные расширения",
          link: "/manual/extensions/",
          items: [
            { text: "Обзор", link: "/manual/extensions/" },
            { text: "Eden — заметки", link: "/manual/extensions/eden" },
            { text: "Delphi — задачи", link: "/manual/extensions/delphi" },
            { text: "Horologion — время", link: "/manual/extensions/horologion" },
            { text: "Arrancador — игры", link: "/manual/extensions/arrancador" },
          ],
        },
      ],
      "/whats-new/": [
        {
          text: "Что нового",
          items: [
            { text: "Обзор", link: "/whats-new/" },
            { text: "Kepler", link: "/whats-new/kepler" },
            { text: "Расширения", link: "/whats-new/extensions" },
          ],
        },
      ],
      "/guide/": [
        {
          text: "Старт",
          items: [
            { text: "Что такое Kosmos", link: "/guide/getting-started" },
            { text: "Структура репозитория", link: "/guide/layout" },
            { text: "Стек и инструменты", link: "/guide/tooling" },
            { text: "Рабочий процесс", link: "/guide/workflow" },
          ],
        },
      ],
      "/concepts/": [
        {
          text: "Концепты",
          items: [
            { text: "Архитектура", link: "/concepts/architecture" },
            { text: "App Index", link: "/concepts/app-index" },
            { text: "Модель данных ARK", link: "/concepts/ark-objects" },
            { text: "Синхронизация", link: "/concepts/sync" },
            { text: "Граница записи в ARK", link: "/concepts/write-boundary" },
            { text: "Read-only SQL", link: "/concepts/readonly-sql" },
            { text: "Command bus", link: "/concepts/command-bus" },
            { text: "Extension host", link: "/concepts/extension-host" },
            { text: "Extension dev mode", link: "/concepts/extension-dev-mode" },
            { text: "Extension installer", link: "/concepts/extension-installer" },
            { text: "Distribution", link: "/concepts/distribution" },
            { text: "RAM benchmarks", link: "/concepts/ram-benchmarks" },
            { text: "Proof loop", link: "/concepts/proof-loop" },
            { text: "Изоляция тестовых БД", link: "/concepts/test-isolation" },
            { text: "Instance slots (prod/dev/multi-dev)", link: "/concepts/instances" },
          ],
        },
      ],
      "/apps/": [
        {
          text: "Приложения",
          items: [
            { text: "Обзор", link: "/apps/" },
            { text: "Kepler — лаунчер", link: "/apps/kepler" },
            { text: "Kepler — Roadmap", link: "/apps/kepler-roadmap" },
            { text: "Eden — заметки", link: "/apps/eden" },
            { text: "Delphi — задачи", link: "/apps/delphi" },
            { text: "Arrancador — игры", link: "/apps/arrancador" },
            { text: "Akasha — EPUB", link: "/apps/akasha" },
            { text: "Dashboard — аналитика", link: "/apps/dashboard" },
            { text: "Horologion — время (WIP)", link: "/apps/horologion" },
            { text: "Horologion — Roadmap", link: "/apps/horologion-roadmap" },
            { text: "Digital Cave — фокус (TBD)", link: "/apps/digital-cave" },
            { text: "Kerux — голос (TBD)", link: "/apps/kerux" },
            { text: "ark-service (Android)", link: "/apps/ark-service" },
          ],
        },
      ],
      "/packages/": [
        {
          text: "Пакеты",
          items: [
            { text: "Обзор", link: "/packages/" },
            { text: "ark-core (Rust runtime)", link: "/packages/ark-core" },
            { text: "@kosmos/ark (TS SDK)", link: "/packages/ark" },
            { text: "@kosmos/visuals", link: "/packages/visuals" },
          ],
        },
        {
          text: "Сервисы",
          items: [
            { text: "Обзор", link: "/services/" },
            { text: "usage-tracker", link: "/services/usage-tracker" },
            { text: "ark-relay-server", link: "/services/ark-relay-server" },
          ],
        },
      ],
      "/services/": [
        {
          text: "Сервисы",
          items: [
            { text: "Обзор", link: "/services/" },
            { text: "usage-tracker", link: "/services/usage-tracker" },
            { text: "ark-relay-server", link: "/services/ark-relay-server" },
          ],
        },
      ],
      "/experiments/": [
        {
          text: "Эксперименты",
          items: [
            { text: "Обзор", link: "/experiments/" },
            { text: "Tauri vs Electron (2026-05)", link: "/experiments/tauri-vs-electron" },
          ],
        },
      ],
      "/reference/": [
        {
          text: "Справочник",
          items: [
            { text: "Правила репозитория", link: "/reference/rules" },
            { text: "Smoke-матрица ARK", link: "/reference/smoke-matrix" },
            { text: "Журнал решений (ADR)", link: "/reference/decisions" },
            { text: "Глоссарий", link: "/reference/glossary" },
            { text: "Команды и скрипты", link: "/reference/commands" },
            { text: "llms.txt (manifest)", link: "/llms.txt", target: "_blank" },
            { text: "full-llms.txt (всё inline)", link: "/full-llms.txt", target: "_blank" },
          ],
        },
      ],
      "/agents/": [
        {
          text: "Для AI-агента",
          items: [
            { text: "Старт работы", link: "/agents/" },
            { text: "Чек-листы по областям", link: "/agents/checklists" },
            { text: "Запреты и гварды", link: "/agents/forbidden" },
            { text: "Поддержка документации", link: "/agents/docs-maintenance" },
            { text: "Шаблоны спецификаций", link: "/agents/spec-templates" },
            { text: "Testing", link: "/agents/testing" },
            { text: "Estimation", link: "/agents/estimation" },
            { text: "Manual tests waiting", link: "/agents/manual-tests-pending" },
            { text: "Постмортемы багов", link: "/agents/postmortems" },
          ],
        },
      ],
    },

    search: {
      provider: "local",
      options: {
        locales: {
          root: {
            translations: {
              button: {
                buttonText: "Поиск",
                buttonAriaLabel: "Поиск по документации",
              },
              modal: {
                noResultsText: "Ничего не найдено по запросу",
                resetButtonTitle: "Очистить",
                displayDetails: "Показать детали",
                backButtonTitle: "Назад",
                footer: {
                  selectText: "выбрать",
                  selectKeyAriaLabel: "Enter",
                  navigateText: "навигация",
                  navigateUpKeyAriaLabel: "стрелка вверх",
                  navigateDownKeyAriaLabel: "стрелка вниз",
                  closeText: "закрыть",
                  closeKeyAriaLabel: "Esc",
                },
              },
            },
          },
        },
      },
    },

    outline: { level: [2, 3], label: "На странице" },
    docFooter: { prev: "← Назад", next: "Дальше →" },
    lastUpdatedText: "Обновлено",
    darkModeSwitchLabel: "Тема",
    sidebarMenuLabel: "Меню",
    returnToTopLabel: "Наверх",
    notFound: {
      title: "Страница не найдена",
      quote: "Заглянула не туда, не та галактика.",
      linkLabel: "На главную",
      linkText: "На главную",
    },

    socialLinks: [],
  },
});
