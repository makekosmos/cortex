import vue from "@vitejs/plugin-vue";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

const rootDir = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig(async ({ mode }) => {
  const plugins = [vue({ features: { vaporInterop: true } })];
  const htmlEntry = process.env.VITE_HTML_ENTRY;

  if (mode !== "test" && !process.env.VITEST) {
    const { default: tailwindcss } = await import("@tailwindcss/vite");
    plugins.push(tailwindcss());
  }

  return {
    base: "./",
    plugins,

    resolve: {
      alias: [
        { find: "@", replacement: path.resolve(rootDir, "./src") },
        { find: "@vue-app", replacement: path.resolve(rootDir, "./src-vue") },
        {
          find: "@kepler/visuals",
          replacement: path.resolve(rootDir, "../../packages/kepler-visuals"),
        },
      ],
      dedupe: ["vue", "vue-router"],
    },

    build: {
      outDir: "out/renderer",
      ...(htmlEntry
        ? {
            rollupOptions: {
              input: path.resolve(rootDir, htmlEntry),
            },
          }
        : {}),
    },

    preview: {
      host: "127.0.0.1",
      port: 4174,
      strictPort: true,
    },

    test: {
      environment: "jsdom",
      globals: true,
      setupFiles: ["./src-vue/test/setup.ts"],
      alias: {
        "@vue-app": path.resolve(rootDir, "./src-vue"),
      },
      include: ["src-vue/test/**/*.{test,spec}.{ts,tsx}"],
      exclude: ["e2e/**"],
      coverage: {
        provider: "v8",
        reporter: ["text", "lcov"],
        reportsDirectory: "coverage",
        include: ["src-vue/**/*.{ts,tsx,vue}"],
        exclude: ["src-vue/test/**", "src-vue/**/*.d.ts", "**/*.css"],
        thresholds: {
          lines: 100,
          functions: 100,
          branches: 100,
          statements: 100,
        },
      },
    },
  };
});
