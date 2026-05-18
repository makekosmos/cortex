import type { StorybookConfig } from "@storybook/vue3-vite";

const config: StorybookConfig = {
  framework: {
    name: "@storybook/vue3-vite",
    options: {},
  },

  stories: [
    "../components/**/*.mdx",
    "../components/**/*.stories.@(js|jsx|mjs|ts|tsx)",
  ],

  addons: [
    "@storybook/addon-essentials",
    "@storybook/addon-interactions",
  ],

  docs: {
    autodocs: "tag",
  },

  core: {
    disableTelemetry: true,
  },

  // Vite 8 / rolldown overrides уже идут от root workspace; Storybook
  // подхватывает локальный vite.config если есть, иначе использует свой
  // дефолт. Inline noExternal для пакетов которые vite-node не может
  // resolve при bun-isolated node_modules (аналогично histoire.config.ts).
  async viteFinal(config) {
    config.optimizeDeps = config.optimizeDeps ?? {};
    config.optimizeDeps.include = [
      ...(config.optimizeDeps.include ?? []),
      "lucide-vue-next",
      "vue-router",
    ];
    config.ssr = config.ssr ?? {};
    (config.ssr as { noExternal?: string[] }).noExternal = [
      "lucide-vue-next",
      "@fontsource-variable/inter",
      "vue-router",
    ];
    return config;
  },
};

export default config;
