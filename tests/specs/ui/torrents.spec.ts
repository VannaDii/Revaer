import { test, expect } from '../../fixtures/app';
import { TorrentsPage } from '../../pages/torrents-page';

test.describe('Torrents', () => {
  test('shows list controls', async ({ app, page }) => {
    await app.goto('/torrents');
    const torrents = new TorrentsPage(page);
    await torrents.expectLoaded();
  });

  test('opens add and create modals', async ({ app, page }) => {
    await app.goto('/torrents');
    const torrents = new TorrentsPage(page);
    await torrents.expectLoaded();

    await torrents.openAddModal();
    await torrents.closeModal();

    await torrents.openCreateModal();
    await torrents.closeModal();
  });

  test('bulk actions stay below topbar menus', async ({ app, page }) => {
    await app.goto('/torrents');
    const torrents = new TorrentsPage(page);
    await torrents.expectLoaded();

    const bulkBar = page.getByTestId('torrents-bulk-action-bar');
    await bulkBar.scrollIntoViewIfNeeded();
    await page.locator('[aria-label="Locale"]').click();
    const localeMenu = page.locator('.locale-menu__content');
    await localeMenu.waitFor({ state: 'visible' });
    const menuBox = await localeMenu.boundingBox();
    expect(menuBox).not.toBeNull();
    if (menuBox) {
      const point = {
        x: menuBox.x + menuBox.width / 2,
        y: menuBox.y + Math.min(8, menuBox.height / 2),
      };
      const isMenuTop = await page.evaluate(({ x, y }) => {
        const el = document.elementFromPoint(x, y);
        return !!el?.closest('.locale-menu__content');
      }, point);
      expect(isMenuTop).toBe(true);
    }

    await page.locator('[aria-label="Server menu"]').click();
    const serverMenu = page.locator('.server-menu__content');
    await serverMenu.waitFor({ state: 'visible' });
    const serverBox = await serverMenu.boundingBox();
    expect(serverBox).not.toBeNull();
    if (serverBox) {
      const point = {
        x: serverBox.x + serverBox.width / 2,
        y: serverBox.y + Math.min(8, serverBox.height / 2),
      };
      const isMenuTop = await page.evaluate(({ x, y }) => {
        const el = document.elementFromPoint(x, y);
        return !!el?.closest('.server-menu__content');
      }, point);
      expect(isMenuTop).toBe(true);
    }
  });
});
