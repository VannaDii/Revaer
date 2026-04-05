import { test } from '../../fixtures/app';
import { HealthPage } from '../../pages/health-page';

test.describe('Health', () => {
  test('renders without page heading context', async ({ app, page }) => {
    await app.goto('/health');
    const health = new HealthPage(page);
    await health.expectLoaded();
  });
});
