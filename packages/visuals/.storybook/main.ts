import { fileURLToPath } from "node:url";
import { dirname } from "node:path";
import type { StorybookConfig } from "@storybook/vue3-vite";
import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";

const config: StorybookConfig = {
  framework: {
    name: "@storybook/vue3-vite",
    options: {},
  },

  stories: ["../components/**/*.stories.@(js|jsx|mjs|ts|tsx)"],

  addons: [getAbsolutePath("@storybook/addon-docs"), getAbsolutePath("@storybook/addon-vitest")],

  core: {
    disableTelemetry: true,
  },

  // Vite 8 / rolldown overrides уже идут от root workspace; Storybook
  // подхватывает локальный vite.config если есть, иначе использует свой
  // дефолт. Inline noExternal для пакетов которые vite-node не может
  // resolve при bun-isolated node_modules.
  async viteFinal(config) {
    // Storybook v10 upgrade удалил vue plugin из default config —
    // добавляем явно. Без него .vue файлы выдают "Failed to parse source"
    // в vite:import-analysis.
    config.plugins = [...(config.plugins ?? []), vue(), tailwindcss()];

    config.optimizeDeps = config.optimizeDeps ?? {};
    config.optimizeDeps.include = [
      ...(config.optimizeDeps.include ?? []),
      "@lucide/vue",
      "vue-router",
      "tailwindcss",
    ];
    config.ssr = config.ssr ?? {};
    (config.ssr as { noExternal?: string[] }).noExternal = [
      "@lucide/vue",
      "@fontsource-variable/inter",
      "vue-router",
    ];
    return config;
  },
};

export default config;

function getAbsolutePath(value: string): any {
  return dirname(fileURLToPath(import.meta.resolve(`${value}/package.json`)));
}
