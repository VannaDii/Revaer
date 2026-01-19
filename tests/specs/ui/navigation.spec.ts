import { test, expect } from '../../fixtures/app';
import { DashboardPage } from '../../pages/dashboard-page';
import { SettingsPage } from '../../pages/settings-page';
import { TorrentsPage } from '../../pages/torrents-page';

test.describe('Navigation', () => {
  test('sidebar routes to primary sections', async ({ app, page }) => {
    await app.goto('/');

    const dashboard = new DashboardPage(page);
    await dashboard.expectLoaded();

    await app.navigate('Torrents');
    await expect(page).toHaveURL(/\/torrents$/);
    const torrents = new TorrentsPage(page);
    await torrents.expectLoaded();

    await app.navigate('Settings');
    await expect(page).toHaveURL(/\/settings$/);
    const settings = new SettingsPage(page);
    await settings.expectLoaded();
  });

  test('topbar menus and sidebar indicators use icon-only controls', async ({
    app,
    page,
  }) => {
    await app.goto('/');

    const serverIcon = page.getByTestId('server-menu-icon');
    await expect(serverIcon.locator('svg rect')).toHaveCount(3);

    const localeTrigger = page.locator('[aria-label="Locale"]');
    const localeTriggerImg = localeTrigger.locator('img');
    await expect(localeTriggerImg).toHaveAttribute('src', /\/gb\.svg$/);
    await expect(localeTriggerImg).toHaveClass(/rounded-full/);

    await localeTrigger.click();
    const localeMenu = page.locator('.locale-menu__content');
    await expect(localeMenu).toBeVisible();
    await expect(localeMenu.locator('img').first()).toHaveClass(/rounded-full/);
    await expect(localeMenu.locator('img[src*="/gb.svg"]')).toHaveCount(1);

    const indicator = page.locator('#layout-sidebar .sse-indicator');
    await expect(indicator).toHaveAttribute('aria-label', /.+/);
    await expect(indicator).toHaveClass(/btn-circle/);
    await expect(indicator).toHaveClass(/btn-sm/);
    await expect(page.locator('#layout-sidebar .sse-indicator__label')).toHaveCount(0);

    const logoutButton = page.getByRole('button', { name: 'Logout' });
    await expect(logoutButton).toBeVisible();
    await expect(logoutButton).toHaveAttribute('data-tip', 'Logout');
    await expect(logoutButton).not.toHaveText(/Logout/);

    const sidebarBox = await page.locator('#layout-sidebar').boundingBox();
    const logoutBox = await logoutButton.boundingBox();
    const indicatorBox = await indicator.boundingBox();
    expect(sidebarBox).not.toBeNull();
    expect(logoutBox).not.toBeNull();
    expect(indicatorBox).not.toBeNull();
    if (sidebarBox && logoutBox && indicatorBox) {
      expect(logoutBox.x + logoutBox.width).toBeLessThanOrEqual(
        sidebarBox.x + sidebarBox.width + 1,
      );
      expect(Math.abs(logoutBox.width - indicatorBox.width)).toBeLessThanOrEqual(1);
      expect(Math.abs(logoutBox.height - indicatorBox.height)).toBeLessThanOrEqual(1);
    }
  });
});
