import path from "node:path";
import { fileURLToPath } from "node:url";
import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vitest/config";

const rootDir = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  plugins: [vue({ features: { vaporInterop: true } })],
  resolve: {
    alias: {
      "@": path.resolve(rootDir, "./src"),
      "@vue-app": path.resolve(rootDir, "./src-vue"),
      "@kepler/visuals": path.resolve(rootDir, "../../packages/kepler-visuals"),
    },
  },
  test: {
    projects: [
      {
        extends: true,
        test: {
          name: "renderer",
          environment: "jsdom",
          globals: true,
          pool: "threads",
          setupFiles: ["./src-vue/test/setup.ts"],
          include: ["src-vue/test/**/*.{test,spec}.ts"],
          exclude: ["e2e/**"],
        },
      },
      {
        extends: true,
        test: {
          name: "electron-main",
          environment: "node",
          globals: true,
          pool: "threads",
          include: ["electron/main/**/*.test.ts"],
          exclude: ["e2e/**"],
        },
      },
    ],
    coverage: {
      provider: "v8",
      reporter: ["text", "lcov"],
      reportsDirectory: "coverage",
      include: [
        "electron/main/ipc/backup-handlers.ts",
        "electron/main/sidecar/arrancador-sidecar.ts",
        "electron/main/services/backup/copy.ts",
        "electron/main/services/backup/index.ts",
        "electron/main/services/backup/restore.ts",
        "electron/main/services/backup-workflow.ts",
        "electron/main/services/backup-workflow/**/*.ts",
        "electron/main/services/games.ts",
        "src-vue/components/game-detail/**/*.vue",
        "src-vue/components/scan/**/*.vue",
        "src-vue/components/statistics/**/*.vue",
        "src-vue/composables/useGameBackups.ts",
        "src-vue/composables/useGameLaunchFlow.ts",
        "src-vue/composables/useGameMetadataSearch.ts",
        "src-vue/composables/useGameSavePath.ts",
        "src-vue/composables/useLibraryGameImport.ts",
        "src-vue/composables/useLibraryInstallStatus.ts",
        "src-vue/composables/useStatisticsHeatmap.ts",
        "src-vue/composables/useUsageProcessPicker.ts",
        "src-vue/lib/**/*.{ts}",
      ],
      exclude: [
        "**/*.test.ts",
        "**/*.spec.ts",
        "src-vue/test/**",
        "src-vue/**/*.d.ts",
        "**/*.css",
      ],
      thresholds: {
        lines: 80,
        functions: 75,
        branches: 70,
        statements: 80,
      },
    },
  },
});
