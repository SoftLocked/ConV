import { defineConfig } from '@playwright/test';
import { APP_PORT, APP_URL, PREVIEW_PORT, PREVIEW_URL, SIGNAL_PORT } from '../tests/browser/urls';
export default defineConfig({
  testDir: '../tests/browser', timeout: 40_000, workers: 1,
  use: { baseURL: APP_URL, headless: true, trace: 'retain-on-failure' },
  webServer: [
    { command: `node node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port ${PREVIEW_PORT} --strictPort`, url: PREVIEW_URL, reuseExistingServer: false },
    { command: '../target/debug/conv-signaling', url: `http://127.0.0.1:${SIGNAL_PORT}/health`, env: { CONV_ORIGIN: APP_URL, CONV_SIGNAL_BIND: `127.0.0.1:${SIGNAL_PORT}` }, reuseExistingServer: false },
    { command: `node node_modules/vite/bin/vite.js --host 127.0.0.1 --port ${APP_PORT} --strictPort`, url: APP_URL, reuseExistingServer: false,
      env: { VITE_SIGNAL_URL: `ws://127.0.0.1:${SIGNAL_PORT}/signal`, VITE_ICE_URL: `http://127.0.0.1:${SIGNAL_PORT}/ice` } },
  ],
});
