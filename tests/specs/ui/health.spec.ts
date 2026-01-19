import { test, expect } from '../../fixtures/app';

test.describe('Health', () => {
  test('renders without page heading context', async ({ app, page }) => {
    await app.goto('/health');

    const content = page.locator('#layout-content');
    await expect(page.locator('#layout-topbar .breadcrumbs')).toHaveCount(0);
    await expect(content.getByText('System health', { exact: true })).toHaveCount(0);
    await expect(
      content.getByText('Live status for core services and recent snapshots.', { exact: true }),
    ).toHaveCount(0);
    await expect(content.getByText('Metrics', { exact: true })).toBeVisible();
  });
});
