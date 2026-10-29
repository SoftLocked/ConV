import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: '../tests/browser', timeout: 40_000, workers: 1,
  use: { baseURL: 'http://127.0.0.1:5173', headless: true, trace: 'retain-on-failure' },
  webServer: [
    { command: 'node node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port 5174', url: 'http://127.0.0.1:5174', reuseExistingServer: false },
    { command: '../target/debug/conv-signaling', url: 'http://127.0.0.1:3001/health', env: { CONV_ORIGIN: 'http://127.0.0.1:5173' }, reuseExistingServer: false },
    { command: 'node node_modules/vite/bin/vite.js --host 127.0.0.1 --port 5173', url: 'http://127.0.0.1:5173', reuseExistingServer: false },
  ],
});
