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
});
