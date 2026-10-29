import { defineConfig } from '@playwright/test'

import { PORT } from './e2e/altium.ts'

// Runs against the real `altium serve` (debug build by default, see
// e2e/altium.ts) with the front of web/dist. On Windows the installed Edge is
// used, elsewhere Playwright's Chromium (`npx playwright install chromium`).
export default defineConfig({
  testDir: './e2e',
  globalSetup: './e2e/global-setup.ts',
  // The tests share one server and one project: one at a time.
  workers: 1,
  fullyParallel: false,
  timeout: 30_000,
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    channel: process.env.PLAYWRIGHT_CHANNEL ?? (process.platform === 'win32' ? 'msedge' : undefined),
    trace: 'retain-on-failure',
  },
})
