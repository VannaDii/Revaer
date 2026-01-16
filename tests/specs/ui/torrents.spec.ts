import { test } from '../../fixtures/app';
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
});
