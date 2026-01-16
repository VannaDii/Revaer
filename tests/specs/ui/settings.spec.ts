import { test } from '../../fixtures/app';
import { SettingsPage } from '../../pages/settings-page';

test.describe('Settings', () => {
  test('switches between tabs', async ({ app, page }) => {
    await app.goto('/settings');
    const settings = new SettingsPage(page);
    await settings.expectLoaded();

    await settings.selectTab('Downloads');
    await settings.expectConfigPlaceholder();

    await settings.selectTab('Network');
    await settings.expectConfigPlaceholder();
  });
});
