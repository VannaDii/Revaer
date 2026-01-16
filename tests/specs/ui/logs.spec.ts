import { test } from '../../fixtures/app';
import { LogsPage } from '../../pages/logs-page';

test.describe('Logs', () => {
  test('renders log stream shell', async ({ app, page }) => {
    await app.goto('/logs');
    const logs = new LogsPage(page);
    await logs.expectLoaded();
  });
});
