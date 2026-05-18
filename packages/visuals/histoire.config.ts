import { defineConfig } from "histoire";
import { HstVue } from "@histoire/plugin-vue";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [HstVue()],

  setupFile: "./histoire.setup.ts",

  storyMatch: ["stories/**/*.story.vue"],

  theme: {
    title: "@kosmos/visuals",
    favicon: undefined,
    logo: undefined,
    colors: {
      primary: {
        50: "#f7f7f7",
        100: "#ededed",
        200: "#d9d9d9",
        300: "#b3b3b3",
        400: "#8c8c8c",
        500: "#737373",
        600: "#525252",
        700: "#404040",
        800: "#262626",
        900: "#171717",
        950: "#0a0a0a",
      },
    },
    defaultColorScheme: "dark",
    storeColorScheme: true,
  },

  tree: {
    groups: [
      { id: "tokens", title: "Tokens" },
      { id: "primitives", title: "Primitives" },
      { id: "popovers", title: "Popovers & Overlays" },
      { id: "datetime", title: "Date & Time" },
      { id: "patterns", title: "Layout & Shell" },
      { id: "complex", title: "Composite Patterns" },
    ],
  },

  vite: {
    plugins: [vue()],
    optimizeDeps: {
      include: ["lucide-vue-next", "vue-router"],
    },
    // vite-node (story collector) при bun-isolated node_modules не может
    // resolve'ить package.json `module` поля у некоторых пакетов. Inline-их
    // через noExternal — тогда vite-node не пытается их externalize'ить.
    ssr: {
      noExternal: ["lucide-vue-next", "@fontsource-variable/inter", "vue-router"],
    },
  },
});
