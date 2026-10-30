import { APP_URL } from './urls';
import { expect, test } from '@playwright/test';
const room = () => crypto.randomUUID().replaceAll('-', '');

test('Unicode editing is saved and restored after reload', async ({ page }, testInfo) => {
  await page.goto(`/#${room()}`);
  const editor = page.getByRole('textbox', { name: 'Document editor' });
  await expect(editor).toBeVisible();
  await editor.fill('Hello 🦀\né Markdown');
  await expect(page.getByText('Saved on this device', { exact: true })).toBeVisible();
  await page.getByRole('textbox', { name: 'Document name' }).fill('Unicode notes');
  await page.getByRole('textbox', { name: 'Document name' }).press('Tab');
  await expect(page.getByText('Saved on this device', { exact: true })).toBeVisible();
  await page.reload();
  await expect(editor.locator('.cm-line')).toHaveText(['Hello 🦀', 'é Markdown']);
  await expect(page.getByRole('textbox', { name: 'Document name' })).toHaveValue('Unicode notes');
  await page.screenshot({ path: testInfo.outputPath('editor.png'), fullPage: true });
});

test('two independent devices exchange edits and restore a disconnected writer', async ({ browser }) => {
  const a = await browser.newContext(), b = await browser.newContext();
  const alice = await a.newPage(), bob = await b.newPage();
  const url = `${APP_URL}/#${room()}`;
  await alice.goto(url); await bob.goto(url);
  const first = alice.getByRole('textbox', { name: 'Document editor' });
  const second = bob.getByRole('textbox', { name: 'Document editor' });
  await expect(first).toBeVisible(); await expect(second).toBeVisible();
  await first.fill('shared 🦀');
  await expect(second).toHaveText('shared 🦀');
  await bob.close();
  await first.press('End'); await first.pressSequentially(' offline');
  await expect(alice.getByText('Saved on this device', { exact: true })).toBeVisible();
  const returned = await b.newPage(); await returned.goto(url);
  await expect(returned.getByRole('textbox', { name: 'Document editor' })).toHaveText('shared 🦀 offline');
  await a.close(); await b.close();
});

test('simultaneous typing retains both writers', async ({ browser }) => {
  const a = await browser.newContext(), b = await browser.newContext();
  const alice = await a.newPage(), bob = await b.newPage();
  const url = `${APP_URL}/#${room()}`;
  await Promise.all([alice.goto(url), bob.goto(url)]);
  const first = alice.getByRole('textbox', { name: 'Document editor' });
  const second = bob.getByRole('textbox', { name: 'Document editor' });
  await Promise.all([expect(first).toBeVisible(), expect(second).toBeVisible()]);
  await Promise.all([first.pressSequentially('alpha'), second.pressSequentially('beta')]);
  await expect.poll(async () => (await first.textContent()) === (await second.textContent())).toBe(true);
  const text = (await first.textContent())!;
  for (const char of 'alphabeta') expect(text.includes(char)).toBe(true);
  expect(text.length).toBe(9);
  await a.close(); await b.close();
});

test('both sides keep offline edits and converge on reconnection', async ({ browser }) => {
  const a = await browser.newContext(), b = await browser.newContext();
  const alice = await a.newPage(), bob = await b.newPage();
  const url = `${APP_URL}/#${room()}`;
  await Promise.all([alice.goto(url), bob.goto(url)]);
  const first = alice.getByRole('textbox', { name: 'Document editor' });
  const second = bob.getByRole('textbox', { name: 'Document editor' });
  await first.fill('start'); await expect(second).toHaveText('start');
  await bob.getByRole('button', { name: 'Connection details' }).click();
  await bob.getByRole('button', { name: 'Work offline' }).click();
  await first.press('End'); await first.pressSequentially(' ALICE');
  await second.press('End'); await second.pressSequentially(' BOB');
  await expect(second).toHaveText('start BOB');
  await bob.getByRole('button', { name: 'Reconnect', exact: true }).click();
  await expect.poll(async () => first.textContent()).toContain('BOB');
  await expect.poll(async () => second.textContent()).toContain('ALICE');
  await expect.poll(async () => (await first.textContent()) === (await second.textContent())).toBe(true);
  await a.close(); await b.close();
});

test('healthy peer channels survive signaling loss', async ({ browser }) => {
  const a = await browser.newContext(), b = await browser.newContext();
  const alice = await a.newPage(), bob = await b.newPage();
  const sockets: { close(): void }[] = [];
  let outage = false;
  for (const page of [alice, bob]) await page.routeWebSocket('**/signal?*', socket => {
    if (outage) socket.close();
    else { socket.connectToServer(); sockets.push(socket); }
  });
  const url = `${APP_URL}/#${room()}`;
  await Promise.all([alice.goto(url), bob.goto(url)]);
  const first = alice.getByRole('textbox', { name: 'Document editor' });
  const second = bob.getByRole('textbox', { name: 'Document editor' });
  await first.fill('connected'); await expect(second).toHaveText('connected');
  outage = true; for (const socket of sockets) socket.close();
  await first.press('End'); await first.pressSequentially(' still editing');
  await expect(second).toHaveText('connected still editing');
  await a.close(); await b.close();
});

test('eight browser replicas converge', async ({ browser }) => {
  const contexts = await Promise.all(Array.from({ length: 8 }, () => browser.newContext()));
  const pages = await Promise.all(contexts.map(context => context.newPage()));
  const url = `${APP_URL}/#${room()}`;
  await Promise.all(pages.map(page => page.goto(url)));
  const editors = pages.map(page => page.getByRole('textbox', { name: 'Document editor' }));
  await Promise.all(editors.map(editor => expect(editor).toBeVisible()));
  await editors[0].fill('eight peers');
  await Promise.all(editors.map(editor => expect(editor).toHaveText('eight peers', { timeout: 15_000 })));
  await Promise.all(contexts.map(context => context.close()));
});

test('forced TURN relay carries document edits', async ({ browser }) => {
  test.skip(!process.env.CONV_TURN_URL, 'Set CONV_TURN_URL and CONV_TURN_SECRET with a running TURN server');
  const a = await browser.newContext(), b = await browser.newContext();
  const alice = await a.newPage(), bob = await b.newPage();
  const url = `${APP_URL}/?relay#${room()}`;
  await Promise.all([alice.goto(url), bob.goto(url)]);
  await alice.getByRole('textbox', { name: 'Document editor' }).fill('through TURN');
  await expect(bob.getByRole('textbox', { name: 'Document editor' })).toHaveText('through TURN', { timeout: 15_000 });
  await expect(bob.getByText('Connected via relay', { exact: true })).toBeVisible();
  await a.close(); await b.close();
});

test('IME composition survives concurrent remote text', async ({ browser }) => {
  const a = await browser.newContext(), b = await browser.newContext();
  const alice = await a.newPage(), bob = await b.newPage();
  const url = `${APP_URL}/#${room()}`;
  await Promise.all([alice.goto(url), bob.goto(url)]);
  const first = alice.getByRole('textbox', { name: 'Document editor' });
  const second = bob.getByRole('textbox', { name: 'Document editor' });
  await expect(first).toBeVisible(); await expect(second).toBeVisible();
  await first.click();
  const input = await a.newCDPSession(alice);
  await input.send('Input.imeSetComposition', { text: 'に', selectionStart: 1, selectionEnd: 1 });
  await second.pressSequentially('remote');
  await input.send('Input.insertText', { text: '日本' });
  await expect.poll(async () => first.textContent()).toContain('日本');
  await expect.poll(async () => second.textContent()).toContain('日本');
  await expect.poll(async () => (await first.textContent()) === (await second.textContent())).toBe(true);
  await first.press('Control+z');
  await expect(first).toContainText('remote');
  await a.close(); await b.close();
});

test('large paste and a versioned backup restore retain the document', async ({ browser }, testInfo) => {
  const source = await browser.newContext(); const page = await source.newPage();
  await page.goto(`${APP_URL}/#${room()}`);
  const text = '🦀'.repeat(8_200);
  await page.getByRole('textbox', { name: 'Document editor' }).fill(text);
  await expect(page.getByText('Saved on this device', { exact: true })).toBeVisible({ timeout: 15_000 });
  const pending = page.waitForEvent('download'); await page.getByRole('button', { name: 'Backup', exact: true }).click();
  const download = await pending; const path = testInfo.outputPath('document.conv'); await download.saveAs(path);
  await source.close();
  const restored = await browser.newContext(); const other = await restored.newPage();
  await other.goto(`${APP_URL}/#${room()}`);
  await expect(other.getByRole('textbox', { name: 'Document editor' })).toBeVisible();
  await other.getByLabel('Restore', { exact: true }).setInputFiles(path);
  await expect(other.getByRole('textbox', { name: 'Document editor' })).toHaveText(text, { timeout: 15_000 });
  await restored.close();
});
