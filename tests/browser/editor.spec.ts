import { expect, test } from '@playwright/test';
const room = () => crypto.randomUUID().replaceAll('-', '');

test('Unicode editing is saved and restored after reload', async ({ page }) => {
  await page.goto(`/#${room()}`);
  const editor = page.getByRole('textbox', { name: 'Document editor' });
  await expect(editor).toBeVisible();
  await editor.fill('Hello 🦀\né Markdown');
  await expect(page.getByText('Saved on this device', { exact: true })).toBeVisible();
  await page.reload();
  await expect(editor.locator('.cm-line')).toHaveText(['Hello 🦀', 'é Markdown']);
});

test('two independent devices exchange edits and restore a disconnected writer', async ({ browser }) => {
  const a = await browser.newContext(), b = await browser.newContext();
  const alice = await a.newPage(), bob = await b.newPage();
  const url = `http://127.0.0.1:5173/#${room()}`;
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
  const url = `http://127.0.0.1:5173/#${room()}`;
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
