import { expect, Page } from '@playwright/test';

export class SettingsPage {
  constructor(private readonly page: Page) {}

  async expectLoaded(): Promise<void> {
    const content = this.page.locator('#layout-content');
    await expect(content.getByText('Settings', { exact: true })).toBeVisible();
    await expect(content.getByText('Connection / Auth', { exact: true })).toBeVisible();
  }

  async selectTab(label: string): Promise<void> {
    const tab = this.page.getByRole('tab', { name: label });
    await tab.click();
    await expect(tab).toHaveAttribute('aria-selected', 'true');
  }

  async expectConfigPlaceholder(): Promise<void> {
    await expect(
      this.page.getByText('Configuration snapshot is not available.', { exact: true }),
    ).toBeVisible();
  }
}
