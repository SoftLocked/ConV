import { test, expect } from '@playwright/test';
test('cached app reloads and edits while the browser is offline', async ({ browser }) => {
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto(`http://127.0.0.1:5174/#${crypto.randomUUID().replaceAll('-', '')}`);
  const editor = page.getByRole('textbox', { name: 'Document editor' });
  await editor.fill('saved before offline');
  await expect(page.getByText('Saved on this device', { exact: true })).toBeVisible();
  await page.evaluate(() => navigator.serviceWorker.ready.then(() => true));
  await page.evaluate(() => navigator.serviceWorker.controller ? true : new Promise(resolve => navigator.serviceWorker.addEventListener('controllerchange', () => resolve(true), { once: true })));
  await context.setOffline(true);
  await page.reload();
  await expect(editor).toHaveText('saved before offline');
  await editor.press('End'); await editor.pressSequentially(' plus offline');
  await expect(page.getByText('Saved on this device', { exact: true })).toBeVisible();
  await page.reload();
  await expect(editor).toHaveText('saved before offline plus offline');
  await context.close();
});
