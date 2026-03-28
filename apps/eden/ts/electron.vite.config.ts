import { resolve } from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'electron-vite'

export default defineConfig({
  main: {
    resolve: {
      alias: {
        '@': resolve(__dirname, 'src'),
      },
    },
    build: {
      outDir: 'dist-electron',
      rollupOptions: {
        input: resolve(__dirname, 'main/main.ts'),
        output: {
          entryFileNames: 'main.js',
          format: 'es',
        },
      },
      externalizeDeps: true,
    },
  },
  preload: {
    resolve: {
      alias: {
        '@': resolve(__dirname, 'src'),
      },
    },
    build: {
      outDir: 'dist-electron',
      emptyOutDir: false,
      rollupOptions: {
        input: resolve(__dirname, 'main/preload.ts'),
        output: {
          entryFileNames: 'preload.js',
          format: 'cjs',
        },
      },
      externalizeDeps: true,
    },
  },
  renderer: {
    root: __dirname,
    resolve: {
      alias: {
        '@': resolve(__dirname, 'src'),
      },
    },
    plugins: [react()],
    build: {
      outDir: 'dist',
      rollupOptions: {
        input: resolve(__dirname, 'index.html'),
      },
    },
  },
})
