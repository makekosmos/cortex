import type { Preview } from "@storybook/vue3-vite";
import { h } from "vue";
import { create } from "storybook/theming";

// Global tokens / theme variables — обязательно для всех stories.
// `css-variables.css` сам загружает Inter Variable + IBM Plex Mono через
// fontsource @import'ы — единый источник правды для шрифтов visuals.
import "../theme/storybook.css";

// Dark theme для Docs page (autodocs) — синхронизирован с manager.ts.
// Без этого argTypes table / story sub-nav рендерятся на белом.
const keplerDocsTheme = create({
  base: "dark",
  appBg: "#1a1a1a",
  appContentBg: "#1a1a1a",
  appPreviewBg: "#1a1a1a",
  appBorderColor: "#2d2d2d",
  appBorderRadius: 8,
  textColor: "#ededed",
  textMutedColor: "#9ca3af",
  textInverseColor: "#1a1a1a",
  barBg: "#1a1a1a",
  barTextColor: "#9ca3af",
  barHoverColor: "#3b82f6",
  barSelectedColor: "#ededed",
  colorPrimary: "#3b82f6",
  colorSecondary: "#3b82f6",
  inputBg: "#252525",
  inputBorder: "#2d2d2d",
  inputTextColor: "#ededed",
  inputBorderRadius: 6,
  fontBase: '"Inter Variable", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
  fontCode: '"IBM Plex Mono", ui-monospace, Consolas, monospace',
});

// --- Kepler viewports ---------------------------------------------------
// Фиксированные размеры окон лаунчера / extension'ов. Полезно при review
// stories — посмотреть, как компонент ведёт себя в реальном frame'е,
// а не в произвольной ширине docs-страницы.
const keplerViewports = {
  launcher: {
    name: "Kepler Launcher (720×460)",
    styles: { width: "720px", height: "460px" },
    type: "desktop" as const,
  },
  settings: {
    name: "Kepler Settings (880×560)",
    styles: { width: "880px", height: "560px" },
    type: "desktop" as const,
  },
  extension: {
    name: "Extension default (1200×800)",
    styles: { width: "1200px", height: "800px" },
    type: "desktop" as const,
  },
};

const preview: Preview = {
  parameters: {
    backgrounds: {
      options: {
        "kepler-dark": // Подвязано к --background из theme/css-variables.css (dark).
          { name: "kepler-dark", value: "oklch(0.145 0 0)" },

        "kepler-light": // TODO(light-theme): когда появится light theme — подвязать сюда
          //   реальное значение --background светлой темы.
          { name: "kepler-light", value: "oklch(1 0 0)" },
      },
    },
    viewport: {
      options: keplerViewports,
    },
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
      expanded: true,
    },
    docs: {
      theme: keplerDocsTheme,
      toc: true,
      source: {
        // Source code блок в docs скрыт по умолчанию — он замусоривает страницу.
        // Открывается явно через "Show code" в каждой story.
        state: "closed",
      },
      story: {
        // Каждая story в docs view получает inline iframe, не открывается в новой вкладке —
        // якорные ссылки на конкретную story работают.
        inline: true,
      },
    },
    options: {
      storySort: {
        // Логический порядок: токены → low-level → composed UI → context demos.
        order: [
          "Intro",
          "Tokens",
          "Inputs", // Toggle, Dropdown, DateChip, calendar…
          "Display", // StatusDot, EmptyState, BlocklistCard, GamePosterCard
          "Lists", // TodoRow
          "Overlays", // Modal, ContextMenu, CommandPalette, QuickEntryPanel
          "Window", // DesktopChrome, Titlebar — chrome для окон
          "Patterns", // composed examples — title bar в реальном окне etc.
          "*",
        ],
      },
    },
  },

  decorators: [
    (story, context) => {
      // Kepler CSS variables живут под селектором `.dark` — оборачиваем
      // story в div.dark. Light theme — TODO (см. globalTypes ниже).
      const theme = (context.globals?.theme as string) ?? "dark";
      if (typeof document !== "undefined") {
        document.documentElement.classList.toggle("dark", theme === "dark");
        document.documentElement.classList.toggle("light", theme === "light");
        document.body.classList.toggle("dark", theme === "dark");
      }

      // Padding только когда layout НЕ fullscreen — fullscreen stories
      // (DesktopChrome / overlays) занимают весь viewport сами.
      const layout = (context.parameters?.layout as string) ?? "padded";
      const isFullscreen = layout === "fullscreen";

      return () =>
        h(
          "div",
          {
            class: theme === "dark" ? "dark" : "light",
            style: {
              position: "relative",
              padding: isFullscreen ? "0" : "16px",
              color: "var(--foreground)",
              background: "var(--background)",
              fontFamily: "var(--font-sans)",
            },
          },
          [h(story())],
        );
    },
  ],

  globalTypes: {
    theme: {
      name: "Тема",
      description: "Тема Kepler (пока только dark)",
      defaultValue: "dark",
      toolbar: {
        icon: "circlehollow",
        // TODO(light-theme): когда появится light theme — раскомментировать
        //   светлый item ниже. Сейчас "Dark only", чтобы не сбивать review
        //   на нерабочем светлом варианте.
        items: [
          { value: "dark", title: "Тёмная" },
          // { value: "light", title: "Светлая (TBD)" },
        ],
        dynamicTitle: true,
      },
    },
  },

  initialGlobals: {
    viewport: {
      value: "extension",
      isRotated: false,
    },

    backgrounds: {
      value: "kepler-dark",
    },
  },
};

export default preview;
