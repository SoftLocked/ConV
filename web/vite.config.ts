import { defineConfig } from 'vite';
export default defineConfig({ server: { proxy: {
  '/signal': { target: 'ws://127.0.0.1:3001', ws: true },
  '/ice': { target: 'http://127.0.0.1:3001' },
} } });
