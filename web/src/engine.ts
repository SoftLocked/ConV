import init, { Engine } from './generated/conv_wasm';
export { Engine };
export const ready = init();
export type Frame = { to: string; data: number[] };
export type Update = { text: string; history: number; outbound: Frame[] };
export const randomId = () => Array.from(crypto.getRandomValues(new Uint8Array(16)), b => b.toString(16).padStart(2, '0')).join('');
export function now() { return BigInt(Math.floor(performance.now())); }
