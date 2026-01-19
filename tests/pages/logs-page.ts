import { expect, Page } from '@playwright/test';

export class LogsPage {
  constructor(private readonly page: Page) {}

  async expectLoaded(): Promise<void> {
    const content = this.page.locator('#layout-content');
    await expect(
      content.locator('span.badge', { hasText: /^(Connecting|Error|Live)$/ }),
    ).toBeVisible();
    await expect(content.locator('.log-terminal')).toBeVisible();
    await expect(this.page.locator('#layout-topbar .breadcrumbs')).toHaveCount(0);
    await expect(
      content.getByText('Live server output streamed on demand.', { exact: true }),
    ).toHaveCount(0);
  }

  async expectFilterControls(): Promise<void> {
    const filter = this.page.getByTestId('logs-level-filter');
    await expect(filter.locator('input[type="radio"]')).toHaveCount(6);
  }

  async selectFilter(label: string): Promise<void> {
    await this.page.getByRole('radio', { name: label }).click();
  }

  async expectLogVisible(text: string): Promise<void> {
    await expect(this.page.locator('.log-terminal')).toContainText(text);
  }

  async expectLogHidden(text: string): Promise<void> {
    await expect(this.page.locator('.log-terminal')).not.toContainText(text);
  }

  async expectTerminalExpanded(minHeight = 160): Promise<void> {
    const terminal = this.page.locator('.log-terminal');
    const box = await terminal.boundingBox();
    expect(box).not.toBeNull();
    if (box) {
      expect(box.height).toBeGreaterThan(minHeight);
    }
  }

  async search(text: string): Promise<void> {
    const input = this.page.getByRole('searchbox');
    await input.fill(text);
  }
}
