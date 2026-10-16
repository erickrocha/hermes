import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './e2e/billing',
  globalSetup: './e2e/billing/setup.ts',
  fullyParallel: false,
  workers: 1,
  reporter: 'list',
  use: {
    baseURL: process.env.BILLING_BACKOFFICE_URL || 'http://localhost:5173',
    headless: true,
    trace: 'retain-on-failure',
  },
})