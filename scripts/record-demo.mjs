// Records actual browser collaboration and a separate live native UDP experiment.
// Run after building WASM, signaling and release native peers.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, readFile, rename } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../', import.meta.url));
const output = new URL('../demo/', import.meta.url); await mkdir(output, { recursive: true });
const children = [];
function start(command, args, options = {}) {
  const child = spawn(command, args, { cwd: root, stdio: ['ignore', 'pipe', 'pipe'], ...options }); children.push(child); return child;
}
async function stop(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const done = once(child, 'exit'); child.kill('SIGTERM'); await done;
}
async function wait(url) {
  for (let n = 0; n < 100; n++) { try { if ((await fetch(url)).ok) return; } catch {} await new Promise(resolve => setTimeout(resolve, 100)); }
  throw new Error(`Server did not start: ${url}`);
}
let browser;
try {
  const signaling = start('target/debug/conv-signaling', [], { env: { ...process.env, CONV_SIGNAL_BIND: '127.0.0.1:3005', CONV_ORIGIN: 'http://127.0.0.1:5175' } });
  const vite = start(process.execPath, ['node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '5175'], {
    cwd: root + 'web', env: { ...process.env, VITE_SIGNAL_URL: 'ws://127.0.0.1:3005/signal', VITE_ICE_URL: 'http://127.0.0.1:3005/ice' },
  });
  await wait('http://127.0.0.1:3005/health'); await wait('http://127.0.0.1:5175');
  browser = await chromium.launch();
  const a = await browser.newContext({ viewport: { width: 1280, height: 800 }, recordVideo: { dir: fileURLToPath(output), size: { width: 1280, height: 800 } } });
  const b = await browser.newContext();
  await a.addInitScript(() => sessionStorage.setItem('conv-name', 'Alice'));
  await b.addInitScript(() => sessionStorage.setItem('conv-name', 'Bob'));
  const alice = await a.newPage(), bob = await b.newPage(); const video = alice.video();
  const url = `http://127.0.0.1:5175/#${crypto.randomUUID().replaceAll('-', '')}`;
  await Promise.all([alice.goto(url), bob.goto(url)]);
  const first = alice.getByRole('textbox', { name: 'Document editor' }), second = bob.getByRole('textbox', { name: 'Document editor' });
  await expect(first).toBeVisible(); await expect(second).toBeVisible();
  await alice.getByRole('textbox', { name: 'Document name' }).fill('A conversation, kept on our devices');
  await first.fill('# ConV field notes\n\nTwo replicas. One shared document.\n');
  await expect(second).toContainText('Two replicas');
  await bob.getByRole('button', { name: 'Connection details' }).click();
  await bob.getByRole('button', { name: 'Work offline' }).click();
  await first.press('Control+End'); await first.pressSequentially('\nAlice keeps writing while Bob is offline.\n', { delay: 30 });
  await second.press('Control+End'); await second.pressSequentially('\nBob has an offline idea worth keeping.\n', { delay: 20 });
  await bob.getByRole('button', { name: 'Reconnect', exact: true }).click();
  await expect(first).toContainText('Bob has an offline idea', { timeout: 15_000 });
  await expect(second).toContainText('Alice keeps writing');
  await alice.getByRole('button', { name: 'Connection details' }).click();
  await stop(signaling);
  await first.press('Control+End'); await first.pressSequentially('\nSignaling is stopped. Our established peer connection still works.\n', { delay: 30 });
  await expect(second).toContainText('Our established peer connection still works');
  await alice.waitForTimeout(1_500);
  await a.close(); await b.close();
  await rename(await video.path(), new URL('browser-collaboration.webm', output));
  await stop(vite);

  const lab = await browser.newContext({ viewport: { width: 1280, height: 800 }, recordVideo: { dir: fileURLToPath(output), size: { width: 1280, height: 800 } } });
  const terminal = await lab.newPage(); const labVideo = terminal.video();
  await terminal.setContent('<html><head><title>ConV native UDP experiment</title></head><body style="background:#14291f;color:#e9f3e9;font:18px/1.8 monospace;padding:45px"><h1>ConV · native UDP experiment</h1><p>200 real UDP sockets on loopback. Tokio tasks in one process.</p><pre></pre></body></html>');
  const arguments_ = ['--peers', '200', '--writers', '10', '--seconds', '5', '--settle-seconds', '8', '--output', 'demo/native.json'];
  await terminal.locator('pre').evaluate((node, command) => { node.textContent = command; }, '$ conv-bench ' + arguments_.join(' ') + '\n\n');
  const experiment = start('target/release/conv-bench', arguments_);
  let updates = Promise.resolve();
  experiment.stdout.on('data', bytes => { updates = updates.then(() => terminal.locator('pre').evaluate((node, text) => { node.textContent += text; }, String(bytes))); });
  const [code] = await once(experiment, 'exit'); await updates; if (code !== 0) throw new Error('Native experiment failed');
  const result = JSON.parse(await readFile(new URL('native.json', output)));
  if (!result.converged) throw new Error('Native replicas did not converge');
  const summary = { commit: result.commit, converged: result.converged, peers: result.peers, mutations_per_replica: result.per_node[0].mutations,
    active_window_mean_bytes_per_node_second: result.per_node.reduce((sum, row) => sum + row.active_window_bytes_per_second, 0) / result.peers,
    note: 'Loopback experiment only; not a browser or WAN result.' };
  await terminal.locator('pre').evaluate((node, text) => { node.textContent += '\n' + text; }, JSON.stringify(summary, null, 2));
  await terminal.waitForTimeout(2_500); await lab.close(); await rename(await labVideo.path(), new URL('native-200.webm', output));
  console.log('Recorded demo/browser-collaboration.webm and demo/native-200.webm');
} finally {
  if (browser) await browser.close();
  for (const child of children) await stop(child).catch(() => {});
}
