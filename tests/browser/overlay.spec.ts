import { expect, test } from '@playwright/test';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';

for (const count of [12, 50]) test(`${count} browser replicas collaborate through bounded overlay links`, async ({ browser }, testInfo) => {
  test.skip(count === 50 && !process.env.CONV_STRESS_BROWSER, 'Set CONV_STRESS_BROWSER=1 for the 50-session integration test');
  test.setTimeout(240_000);
  const contexts = await Promise.all(Array.from({ length: count }, () => browser.newContext()));
  try {
    const pages = await Promise.all(contexts.map(context => context.newPage()));
    let outage = false;
    const sockets: { close(): void }[] = [];
    for (const page of pages) await page.routeWebSocket('**/signal?*', socket => {
      if (outage) socket.close(); else { socket.connectToServer(); sockets.push(socket); }
    });
    const room = crypto.randomUUID().replaceAll('-', '');
    const editors = pages.map(page => page.getByRole('textbox', { name: 'Document editor' }));
    for (let index = 0; index < count; index += 5) await Promise.all(pages.slice(index, index + 5).map(async page => {
      await page.goto(`http://127.0.0.1:5173/#${room}`);
      await expect(page.getByRole('textbox', { name: 'Document editor' })).toBeVisible({ timeout: 30_000 });
    }));
    await Promise.all(pages.map(page => page.getByRole('button', { name: 'Connection details' }).click()));
    await expect.poll(async () => Promise.all(pages.map(page => page.locator('.diagnostics').getAttribute('data-participants'))), { timeout: 30_000 }).toEqual(pages.map(() => String(count)));
    await expect.poll(async () => (await Promise.all(pages.map(page => page.locator('.diagnostics').getAttribute('data-direct-links')))).every(value => Number(value) >= 2 && Number(value) <= 8), { timeout: 30_000 }).toBe(true);
    if (count === 50) {
      const capacity = await pages[0].evaluate(room => new Promise((resolve, reject) => {
        const url = new URL('/signal', location.href); url.protocol = 'ws:';
        url.search = new URLSearchParams({ room, session: crypto.randomUUID().replaceAll('-', ''), name: 'Extra participant' }).toString();
        const extra = new WebSocket(url);
        const timeout = setTimeout(() => { extra.close(); reject(new Error('Capacity response missing')); }, 10_000);
        extra.onmessage = ({ data }) => { clearTimeout(timeout); resolve(JSON.parse(String(data))); extra.close(); };
      }), room);
      expect(capacity).toEqual({ type: 'capacity', limit: 50 });
    }
    await Promise.all(editors.map(async (editor, i) => { await editor.press('Control+End'); await pages[i].keyboard.insertText(`<writer-${i}>`); }));
    const markers = Array.from({ length: count }, (_, i) => `<writer-${i}>`);
    await expect.poll(async () => (await Promise.all(editors.map(editor => editor.textContent()))).every(text => markers.every(marker => text!.includes(marker))), { timeout: 45_000 }).toBe(true);
    const departed = count - 1;
    await pages[departed].getByRole('button', { name: 'Work offline' }).click();
    await editors[departed].press('Control+End'); await editors[departed].pressSequentially(' OFFLINE');
    await editors[0].press('Control+End'); await editors[0].pressSequentially(' ONLINE');
    await pages[departed].getByRole('button', { name: 'Reconnect', exact: true }).click();
    await expect.poll(async () => {
      const values = await Promise.all(editors.map(editor => editor.textContent()));
      return values.every(text => text === values[0] && text!.includes('OFFLINE') && text!.includes('ONLINE'));
    }, { timeout: 45_000 }).toBe(true);
    outage = true; for (const socket of sockets) socket.close();
    await editors[0].press('Control+End'); await pages[0].keyboard.insertText(' <after-outage>');
    await expect.poll(async () => {
      const values = await Promise.all(editors.map(editor => editor.textContent()));
      return values.every(value => value === values[0] && value!.includes('<after-outage>'));
    }, { timeout: 45_000 }).toBe(true);
    // Compare complete retained CRDT histories, not just the visible strings.
    const hashes = await Promise.all(pages.map(async page => {
      const pending = page.waitForEvent('download'); await page.getByRole('button', { name: 'Backup', exact: true }).click();
      const download = await pending; const bytes = await readFile((await download.path())!);
      const raw = bytes.subarray(bytes.indexOf(10, 12) + 1);
      return createHash('sha256').update(raw).digest('hex');
    }));
    expect(new Set(hashes).size).toBe(1);
    const degrees = await Promise.all(pages.map(page => page.locator('.diagnostics').getAttribute('data-direct-links')));
    expect(degrees.every(value => Number(value) <= 8)).toBe(true);
    await testInfo.attach('overlay-proof.json', { body: Buffer.from(JSON.stringify({
      commit: execFileSync('git', ['rev-parse', 'HEAD']).toString().trim(), browser: browser.version(),
      sessions: count, isolation: 'independent Chromium contexts', direct_degrees: degrees.map(Number),
      every_writer_contributed: true, offline_rejoin: true, signaling_outage: true,
      complete_crdt_backups_identical: true, backup_sha256: hashes[0],
    }, null, 2)), contentType: 'application/json' });
  } finally { await Promise.all(contexts.map(context => context.close())); }
});
