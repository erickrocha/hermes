import { configDefaults, defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  server: { port: 5173 },
  test: {
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
    exclude: [...configDefaults.exclude, 'e2e/**'],
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          const path = id.replace(/\\/g, '/')
          if (!path.includes('/node_modules/')) return
          if (/\/node_modules\/(react|react-dom|scheduler)\//.test(path)) return 'react-vendor'
          if (/\/node_modules\/react-router(-dom)?\//.test(path)) return 'router-vendor'
          if (/\/node_modules\/(?:@reduxjs|redux|redux-thunk|immer|reselect)\//.test(path)) return 'state-vendor'
          if (/\/node_modules\/@tanstack\//.test(path)) return 'table-vendor'
          if (/\/node_modules\/(i18next|i18next-browser-languagedetector|react-i18next)\//.test(path)) return 'i18n-vendor'
          if (/\/node_modules\/axios\//.test(path)) return 'http-vendor'
        },
      },
    },
  },
})
