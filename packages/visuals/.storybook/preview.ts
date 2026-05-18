import type { Preview } from "@storybook/vue3";
import { h } from "vue";

// Global tokens / theme variables — обязательно для всех stories.
// Совпадает с histoire.setup.ts чтобы preview между двумя инструментами
// смотрелся одинаково.
import "@fontsource-variable/inter";
import "../theme/css-variables.css";
import "../components/sidebar.css";

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

// --- Component status badge ---------------------------------------------
// Convention: каждая *.stories.ts может объявить `parameters.handcrafted`.
//   true  → ✋ HANDCRAFTED (зелёный) — пользователь сам выверил UX.
//   false → 🤖 AGENT-BUILT — собрано агентом, нужна доработка UX (default).
// Badge рендерится поверх preview в правом верхнем углу.
function renderStatusBadge(handcrafted: boolean) {
  const label = handcrafted
    ? "✋ HANDCRAFTED"
    : "🤖 AGENT-BUILT — нужна доработка";
  const bg = handcrafted
    ? "oklch(0.62 0.17 145)" // green
    : "oklch(0.65 0.18 50)"; // amber
  return h(
    "div",
    {
      style: {
        position: "absolute",
        top: "8px",
        right: "8px",
        zIndex: "9999",
        padding: "4px 10px",
        fontSize: "11px",
        fontWeight: "600",
        letterSpacing: "0.02em",
        borderRadius: "999px",
        background: bg,
        color: "#fff",
        boxShadow: "0 2px 8px rgba(0,0,0,0.25)",
        pointerEvents: "none",
        userSelect: "none",
        fontFamily: "var(--font-sans)",
      },
    },
    label,
  );
}

const preview: Preview = {
  parameters: {
    backgrounds: {
      default: "kepler-dark",
      values: [
        // Подвязано к --background из theme/css-variables.css (dark).
        { name: "kepler-dark", value: "oklch(0.145 0 0)" },
        // TODO(light-theme): когда появится light theme — подвязать сюда
        //   реальное значение --background светлой темы.
        { name: "kepler-light", value: "oklch(1 0 0)" },
      ],
    },
    viewport: {
      viewports: keplerViewports,
      defaultViewport: "extension",
    },
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
      expanded: true,
    },
    docs: {
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
        order: ["Intro", "Tokens", "Primitives", "Components", "Patterns", "*"],
      },
    },
    // Default convention: agent-built. Stories помечают handcrafted: true,
    // если пользователь верифицировал UX.
    handcrafted: false,
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

      const handcrafted = context.parameters?.handcrafted === true;

      return () =>
        h(
          "div",
          {
            class: theme === "dark" ? "dark" : "light",
            style: {
              position: "relative",
              padding: "1rem",
              minHeight: "100vh",
              color: "var(--foreground)",
              background: "var(--background)",
              fontFamily: "var(--font-sans)",
            },
          },
          [renderStatusBadge(handcrafted), h(story())],
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
};

export default preview;
