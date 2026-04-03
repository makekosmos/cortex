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
    vue({ features: { vaporInterop: true } }),
    checker({ typescript: { tsconfigPath: './tsconfig.json' } }),
    tailwindcss(),
    ...(!isWeb
      ? [
          electron({
            main: {
              entry: 'electron/main.ts',
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
      '@kepler/visuals': path.resolve(__dirname, '../../../packages/kepler-visuals'),
    },
  },
  clearScreen: false,
});
