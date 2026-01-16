import { expect, Page } from '@playwright/test';

export class LogsPage {
  constructor(private readonly page: Page) {}

  async expectLoaded(): Promise<void> {
    const content = this.page.locator('#layout-content');
    await expect(content.getByText('Logs', { exact: true })).toBeVisible();
    await expect(
      content.getByText('Live server output streamed on demand.', { exact: true }),
    ).toBeVisible();
    await expect(
      content.locator('span.badge', { hasText: /^(Connecting|Error|Live)$/ }),
    ).toBeVisible();
  }
}
