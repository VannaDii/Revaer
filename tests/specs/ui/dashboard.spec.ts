import { test } from '../../fixtures/app';
import { DashboardPage } from '../../pages/dashboard-page';

test.describe('Dashboard', () => {
  test('renders core overview panels', async ({ app, page }) => {
    await app.goto('/');
    const dashboard = new DashboardPage(page);
    await dashboard.expectLoaded();
  });
});
