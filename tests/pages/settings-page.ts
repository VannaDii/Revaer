import { expect, Page } from '@playwright/test';

export class SettingsPage {
  constructor(private readonly page: Page) {}

  async expectLoaded(): Promise<void> {
    const content = this.page.locator('#layout-content');
    const tablist = content.getByRole('tablist');
    await expect(tablist).toBeVisible();
    await expect(tablist).toHaveClass(/tabs-lift/);
    const tabs = tablist.getByRole('tab');
    await expect(tabs).toHaveCount(7);
    await expect(tabs.locator('svg')).toHaveCount(7);
    await expect(content.getByText('Connection / Auth', { exact: true })).toBeVisible();
    const panel = content.getByRole('tabpanel');
    await expect(panel).toBeVisible();
    await expect(panel).toHaveClass(/tab-content/);
    await expect(this.page.locator('#layout-topbar .breadcrumbs')).toHaveCount(0);
    await expect(
      content.locator(
        'text=Configure authentication, engine behavior, and storage policies.',
      ),
    ).toHaveCount(0);
  }

  async selectTab(label: string): Promise<void> {
    const tab = this.page.getByRole('tab', { name: label });
    await tab.click();
    await expect(tab).toHaveAttribute('aria-selected', 'true');
    const tabId = await tab.getAttribute('id');
    if (tabId) {
      const panel = this.page.getByRole('tabpanel');
      await expect(panel).toBeVisible();
      await expect(panel).toHaveAttribute('aria-labelledby', tabId);
    }
  }

  async expectConfigPlaceholder(): Promise<void> {
    await expect(
      this.page.getByText('Configuration snapshot is not available.', { exact: true }),
    ).toBeVisible();
  }
}
