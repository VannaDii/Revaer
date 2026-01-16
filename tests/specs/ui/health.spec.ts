import { test } from '../../fixtures/app';
import { HealthPage } from '../../pages/health-page';

test.describe('Health', () => {
  test('shows health summaries and metrics', async ({ app, page }) => {
    await app.goto('/health');
    const health = new HealthPage(page);
    await health.expectLoaded();
  });
});
