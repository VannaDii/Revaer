import { test, expect } from '../../fixtures/app';
import { randomUUID } from 'crypto';

test.describe('Route coverage', () => {
  test('opens torrent detail route', async ({ app, page }) => {
    const torrentId = randomUUID();
    await app.goto(`/torrents/${torrentId}`);
    await expect(page).toHaveURL(new RegExp(`/torrents/${torrentId}$`));
    const spinner = page.getByRole('status');
    const overviewTab = page.getByRole('tab', { name: 'Overview' });
    await expect.poll(async () => {
      return (await spinner.isVisible()) || (await overviewTab.isVisible());
    }).toBeTruthy();
  });

  test('renders not found placeholder', async ({ app, page }) => {
    await app.goto('/definitely-not-a-route');
    await expect(page.getByText('Not found', { exact: true })).toBeVisible();
  });
});
