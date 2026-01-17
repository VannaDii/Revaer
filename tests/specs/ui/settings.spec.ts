import { test } from '../../fixtures/app';
import { SettingsPage } from '../../pages/settings-page';

test.describe('Settings', () => {
  test('switches between tabs', async ({ app, page }) => {
    await app.goto('/settings');
    const settings = new SettingsPage(page);
    await settings.expectLoaded();

    const tabs = ['Downloads', 'Seeding', 'Network', 'Storage', 'Labels', 'System'];
    for (const tab of tabs) {
      await settings.selectTab(tab);
      await settings.expectConfigPlaceholder();
    }
  });
});
