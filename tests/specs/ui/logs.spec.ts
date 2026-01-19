import { test, expect } from '../../fixtures/app';
import { LogsPage } from '../../pages/logs-page';

async function mockLogStream(page: import('@playwright/test').Page, body: string): Promise<void> {
  await page.route('**/v1/logs/stream', async (route) => {
    const request = route.request();
    const origin = request.headers()['origin'] ?? '*';
    const corsHeaders = {
      'Access-Control-Allow-Origin': origin,
      'Access-Control-Allow-Methods': 'GET, OPTIONS',
      'Access-Control-Allow-Headers':
        request.headers()['access-control-request-headers'] ??
        'authorization, x-revaer-api-key, content-type',
      'Access-Control-Max-Age': '600',
    };
    if (request.method() === 'OPTIONS') {
      await route.fulfill({ status: 204, headers: corsHeaders });
      return;
    }
    await route.fulfill({
      status: 200,
      headers: {
        ...corsHeaders,
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
      },
      body,
    });
  });
}

test.describe('Logs', () => {
  test('renders log stream shell', async ({ app, page }) => {
    await mockLogStream(page, '');
    await app.goto('/logs');
    const logs = new LogsPage(page);
    await logs.expectLoaded();
    await logs.expectTerminalExpanded();
    await expect(page.getByText('No log lines yet.', { exact: true })).toBeVisible();
  });

  test('filters and searches log output', async ({ app, page }) => {
    const sseBody = [
      'data: TRACE trace details\n\n',
      'data: DEBUG debug details\n\n',
      'data: level=INFO info details\n\n',
      'data: level=WARN warn details\n\n',
      'data: level=ERROR error details\n\n',
    ].join('');
    await mockLogStream(page, sseBody);

    await app.goto('/logs');
    const logs = new LogsPage(page);
    await logs.expectLoaded();
    await logs.expectFilterControls();
    await expect(page.locator('[data-testid="logs-search-hint"]')).toHaveCount(0);

    await logs.expectLogVisible('TRACE trace details');
    await logs.expectLogVisible('DEBUG debug details');
    await logs.expectLogVisible('level=INFO info details');
    await logs.expectLogVisible('level=WARN warn details');
    await logs.expectLogVisible('level=ERROR error details');

    await logs.selectFilter('Warn');
    await logs.expectLogVisible('level=WARN warn details');
    await logs.expectLogVisible('level=ERROR error details');
    await logs.expectLogHidden('level=INFO info details');
    await logs.expectLogHidden('DEBUG debug details');

    await logs.selectFilter('All levels');
    await logs.selectFilter('Info');
    await logs.expectLogVisible('level=INFO info details');
    await logs.expectLogVisible('level=WARN warn details');
    await logs.expectLogVisible('level=ERROR error details');
    await logs.expectLogHidden('DEBUG debug details');
    await logs.expectLogHidden('TRACE trace details');

    await logs.search('error');
    const terminal = page.locator('.log-terminal');
    await expect(terminal).toContainText('level=ERROR error details');
    await expect(terminal).not.toContainText('level=WARN warn details');
  });
});
