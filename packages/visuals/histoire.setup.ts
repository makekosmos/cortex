import { defineSetupVue3 } from "@histoire/plugin-vue";
import { createMemoryHistory, createRouter } from "vue-router";

// Global tokens / theme variables — обязательно для всех stories.
import "@fontsource-variable/inter";
import "./theme/css-variables.css";
import "./components/sidebar.css";

// Глобальные стили для preview-канваса (background + текстовый цвет берём из
// CSS-variables, чтобы Histoire light/dark переключение красило preview).
import "./stories/_preview.css";

const stubComponent = {
  template: "<div style=\"padding:1rem;color:var(--muted-foreground);\">route stub</div>",
};

/**
 * Histoire переключает тему через `data-color-mode="dark|light"` на `<html>`.
 * Наши CSS-variables живут под селектором `.dark` — синхронизируем класс на
 * `<body>` через MutationObserver, чтобы preview окрашивался темой.
 */
function bindHistoireTheme() {
  if (typeof document === "undefined") return;
  const apply = () => {
    const mode = document.documentElement.getAttribute("data-color-mode");
    document.body.classList.toggle("dark", mode === "dark");
  };
  apply();
  const observer = new MutationObserver(apply);
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-color-mode"],
  });
}

export const setupVue3 = defineSetupVue3(({ app }) => {
  bindHistoireTheme();

  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: stubComponent },
      { path: "/:pathMatch(.*)*", component: stubComponent },
    ],
  });
  app.use(router);
});
