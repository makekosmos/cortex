import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import checker from 'vite-plugin-checker';
import tailwindcss from '@tailwindcss/vite';
import electron from 'vite-plugin-electron/simple';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const isWeb = process.env.BUILD_TARGET === 'web';

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    vue(),
    checker({ typescript: { tsconfigPath: './tsconfig.json' } }),
    tailwindcss(),
    ...(!isWeb
      ? [
          electron({
            main: {
              entry: 'electron/main.ts',
              vite: {},
            },
            preload: {
              input: 'electron/preload.ts',
            },
          }),
        ]
      : []),
  ],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
      '@kepler/ark': path.resolve(__dirname, '../../../packages/kepler-ark/src/index.ts'),
      '@kepler/visuals': path.resolve(__dirname, '../../../packages/kepler-visuals'),
    },
    dedupe: ['vue', 'vue-router'],
  },
  clearScreen: false,
});
