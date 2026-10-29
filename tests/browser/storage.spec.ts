import { expect, test } from '@playwright/test';

// Exercise the real browser transaction rather than a mocked storage promise.
test('an aborted save leaves the previous durable snapshot intact', async ({ page }) => {
  await page.goto('/');
  const result = await page.evaluate(async () => {
    const { save, load } = await import('/src/storage/local.ts');
    const room = crypto.randomUUID(), key = room + ':test';
    await save({ key, room, name: 'committed', backup: new Uint8Array([1, 2, 3]) });
    const put = IDBObjectStore.prototype.put;
    let rejected = false;
    IDBObjectStore.prototype.put = function (...args) {
      const request = put.apply(this, args);
      this.transaction.abort();
      return request;
    };
    try { await save({ key, room, name: 'uncommitted', backup: new Uint8Array([9]) }); }
    catch { rejected = true; }
    finally { IDBObjectStore.prototype.put = put; }
    const restored = await load(room);
    return { rejected, name: restored[0].name, bytes: Array.from(restored[0].backup), count: restored.length };
  });
  expect(result).toEqual({ rejected: true, name: 'committed', bytes: [1, 2, 3], count: 1 });
});

test('a failed local save is visible and a later edit can persist the retained work', async ({ page, context }) => {
  await context.route('**/src/storage/local.ts', async route => {
    const response = await route.fetch();
    const original = await response.text();
    const signature = 'export async function save(value) {';
    expect(original).toContain(signature);
    const body = original.replace(signature, signature + '\nif (!globalThis.__convFailedOnce) { globalThis.__convFailedOnce = true; throw new DOMException("Injected storage quota failure", "QuotaExceededError"); }');
    await route.fulfill({ response, body });
  });
  await page.goto(`/#${crypto.randomUUID().replaceAll('-', '')}`);
  const editor = page.getByRole('textbox', { name: 'Document editor' });
  await editor.fill('retained after failure');
  await expect(page.getByText('Local save failed', { exact: true })).toBeVisible();
  await expect(page.getByRole('alert')).toContainText('QuotaExceededError');
  await editor.press('End'); await editor.press('!');
  await expect(page.getByText('Saved on this device', { exact: true })).toBeVisible();
  await expect(page.getByRole('alert')).toBeHidden();
  await page.reload();
  await expect(editor).toHaveText('retained after failure!');
});

test('independently offline tabs retain both histories across reload', async ({ context }) => {
  const url = `/#${crypto.randomUUID().replaceAll('-', '')}`;
  const a = await context.newPage(), b = await context.newPage();
  await Promise.all([a.goto(url), b.goto(url)]);
  for (const page of [a, b]) {
    await expect(page.getByRole('textbox', { name: 'Document editor' })).toBeVisible();
    await page.getByRole('button', { name: 'Connection details' }).click();
    await page.getByRole('button', { name: 'Work offline' }).click();
  }
  await a.getByRole('textbox', { name: 'Document editor' }).fill('ALICE');
  await b.getByRole('textbox', { name: 'Document editor' }).fill('BOB');
  for (const page of [a, b]) await expect(page.getByText('Saved on this device', { exact: true })).toBeVisible();
  await a.close(); await b.close();
  const returned = await context.newPage(); await returned.goto(url);
  const editor = returned.getByRole('textbox', { name: 'Document editor' });
  await expect(editor).toContainText('ALICE'); await expect(editor).toContainText('BOB');
  expect((await editor.textContent())!.length).toBe(8);
});
