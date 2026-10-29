import { test, expect } from '@playwright/test';
import { writeFile, mkdir } from 'node:fs/promises';
import { cpus } from 'node:os';
import { execFileSync } from 'node:child_process';
import { performance as controllerClock } from 'node:perf_hooks';
import path from 'node:path';

const enabled = !!process.env.CONV_BENCH_BROWSER;
test('measure isolated WASM merges and foreground editor latency', async ({ browser }, testInfo) => {
  test.skip(!enabled, 'Set CONV_BENCH_BROWSER=1 to run measured experiments');
  test.setTimeout(180_000);
  const context = await browser.newContext(); const page = await context.newPage();
  await page.goto('http://127.0.0.1:5173');
  const editor = page.getByRole('textbox', { name: 'Document editor' }); await expect(editor).toBeVisible();
  const wasm = await page.evaluate(async () => {
    const module = await import('/src/generated/conv_wasm.js'); await module.default();
    let step = Infinity, last = performance.now();
    for (let n = 0; n < 20_000; n++) { const next = performance.now(); if (next > last) step = Math.min(step, next - last); last = next; }
    const cases = [];
    for (const seed of [0, 1, 2]) {
      const experiment = new module.MergeExperiment(102_400, 2_200, false, BigInt(seed));
      for (let n = 0; n < 200; n++) experiment.merge_one();
      const times = [];
      for (let n = 0; n < 2_000; n++) { const before = performance.now(); experiment.merge_one(); times.push(performance.now() - before); }
      const verified = experiment.verify(); if (!verified) throw new Error('WASM merge state mismatch');
      times.sort((a, b) => a - b);
      cases.push({ seed, verified, document_bytes: 102_400, samples: 2_000, warmup: 200, raw_ms: times,
        p50_ms: times[999], p95_ms: times[1899], p99_ms: times[1979], p99_clock_upper_bound_ms: Number.isFinite(step) ? times[1979] + step : null,
        serialized_state_bytes: experiment.serialized_bytes(), tombstones: experiment.tombstones() });
      experiment.free();
    }
    return { cases, observed_clock_step_ms: Number.isFinite(step) ? step : null, method: 'host performance.now around one WASM merge_one; includes WASM boundary; excludes setup, decoding, protocol history, rendering and network' };
  });
  await editor.fill('x'.repeat(1_024));
  await editor.press('End');
  await page.evaluate(() => {
    (window as any).__convPaintSamples = [];
    document.querySelector('.cm-content')!.addEventListener('keydown', () => {
      const start = performance.now(); requestAnimationFrame(() => requestAnimationFrame(() => (window as any).__convPaintSamples.push(performance.now() - start)));
    });
  });
  for (let n = 0; n < 100; n++) await editor.press('a');
  await expect.poll(() => page.evaluate(() => (window as any).__convPaintSamples.length)).toBe(100);
  const local = await page.evaluate(() => [...(window as any).__convPaintSamples].sort((a: number, b: number) => a - b));
  await context.close();
  const contexts = await Promise.all(Array.from({ length: 8 }, () => browser.newContext()));
  const pages = await Promise.all(contexts.map(context => context.newPage()));
  const room = crypto.randomUUID().replaceAll('-', '');
  await Promise.all(pages.map(page => page.goto(`http://127.0.0.1:5173/#${room}`)));
  const editors = pages.map(page => page.getByRole('textbox', { name: 'Document editor' }));
  await Promise.all(editors.map(editor => expect(editor).toBeVisible()));
  await editors[0].fill('base'); await Promise.all(editors.map(editor => expect(editor).toHaveText('base', { timeout: 15_000 })));
  const visibility: number[] = [];
  for (let n = 1; n <= 30; n++) {
    const before = controllerClock.now();
    await editors[0].press('End'); await editors[0].press('z');
    await expect.poll(async () => (await Promise.all(editors.slice(1).map(editor => editor.textContent()))).every(text => text === 'base' + 'z'.repeat(n)), { intervals: [5, 10, 20, 40] }).toBe(true);
    visibility.push(controllerClock.now() - before);
  }
  await Promise.all(contexts.map(context => context.close()));
  visibility.sort((a, b) => a - b);
  const root = path.resolve(testInfo.config.rootDir, '../../benchmarks/raw-results'); await mkdir(root, { recursive: true });
  await writeFile(path.join(root, 'browser.json'), JSON.stringify({ commit: execFileSync('git', ['rev-parse', 'HEAD']).toString().trim(),
    source_dirty: !!execFileSync('git', ['status', '--porcelain', '--untracked-files=no']).toString(), browser: browser.version(), hardware: cpus()[0]?.model, device: 'single WSL/Linux host, headless Chromium, eight independent browser contexts', wasm,
    local_render_proxy: { samples: local, p95_ms: local[94], document_bytes: 1_024, method: 'trusted keydown to second animation-frame callback; upper-bound proxy, not a physical screen measurement' },
    remote_visibility: { samples_ms: visibility, p95_ms: visibility[27], peers: 8, writers: 1, network: 'loopback WebRTC direct', method: 'one controller monotonic clock from keyboard command to all seven remote DOMs; includes automation overhead; waits for prior edit before next' },
  }, null, 2));
});
