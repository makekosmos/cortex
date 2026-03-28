import { test, expect, type ElectronApplication, type Page } from '@playwright/test';
import { _electron as electron } from 'playwright';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');

let app: ElectronApplication;
let page: Page;

test.beforeAll(async () => {
  app = await electron.launch({
    args: [path.join(rootDir, 'dist-electron/main.js')],
    env: {
      ...process.env,
      NODE_ENV: 'production',
    },
  });
  page = await app.firstWindow();

  // Set Ark credentials so the auth overlay doesn't block
  await page.evaluate(() => {
    localStorage.setItem('delphi.ark_url', 'http://localhost:8000');
    localStorage.setItem('delphi.ark_api_key', 'test-key');
  });
  // Reload so credentials take effect
  await page.reload();
  await page.waitForLoadState('domcontentloaded');
});

test.afterAll(async () => {
  await app.close();
});

test.describe('Sidebar navigation', () => {
  test('sidebar is visible and contains all navigation items', async () => {
    const sidebar = page.locator('aside');
    await expect(sidebar).toBeVisible();

    await expect(sidebar.getByText('Входящие')).toBeVisible();
    await expect(sidebar.getByText('Сегодня')).toBeVisible();
    await expect(sidebar.getByText('Планы')).toBeVisible();
    await expect(sidebar.getByText('Журнал')).toBeVisible();
    await expect(sidebar.getByText('Корзина')).toBeVisible();
  });

  test('sidebar links are clickable and navigate correctly', async () => {
    // Default route should be Inbox
    await expect(page.getByRole('heading', { name: 'Входящие' })).toBeVisible();

    // Navigate to Today
    await page.locator('aside').getByText('Сегодня').click();
    await expect(page.getByRole('heading', { name: 'Сегодня' })).toBeVisible();

    // Navigate to Upcoming
    await page.locator('aside').getByText('Планы').click();
    await expect(page.getByRole('heading', { name: 'Планы' })).toBeVisible();

    // Navigate to Logbook
    await page.locator('aside').getByText('Журнал').click();
    await expect(page.getByRole('heading', { name: 'Журнал' })).toBeVisible();

    // Navigate to Trash
    await page.locator('aside').getByText('Корзина').click();
    await expect(page.getByRole('heading', { name: 'Корзина' })).toBeVisible();

    // Navigate back to Inbox
    await page.locator('aside').getByText('Входящие').click();
    await expect(page.getByRole('heading', { name: 'Входящие' })).toBeVisible();
  });

  test('active sidebar item is highlighted', async () => {
    // Navigate to Inbox first
    await page.locator('aside').getByText('Входящие').click();
    const inboxLink = page.locator('aside').getByText('Входящие').locator('..');
    await expect(inboxLink).toHaveClass(/font-medium/);

    // Navigate to Today and check it becomes active
    await page.locator('aside').getByText('Сегодня').click();
    const todayLink = page.locator('aside').getByText('Сегодня').locator('..');
    await expect(todayLink).toHaveClass(/font-medium/);
  });
});
