function port(name: string, fallback: number): number {
  const value = Number(process.env[name] ?? fallback);
  if (!Number.isInteger(value) || value < 1024 || value > 65535) throw new Error(`Invalid ${name}`);
  return value;
}
export const APP_PORT = port('CONV_TEST_APP_PORT', 5173);
export const SIGNAL_PORT = port('CONV_TEST_SIGNAL_PORT', 3001);
export const PREVIEW_PORT = port('CONV_TEST_PREVIEW_PORT', 5174);
export const APP_URL = `http://127.0.0.1:${APP_PORT}`;
export const PREVIEW_URL = `http://127.0.0.1:${PREVIEW_PORT}`;
