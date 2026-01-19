import { expect, Page } from '@playwright/test';

export class DashboardPage {
  constructor(private readonly page: Page) {}

  async expectLoaded(): Promise<void> {
    await expect(this.page.getByText('Storage Status', { exact: true })).toBeVisible();
    await expect(this.page.getByText('Tracker Health', { exact: true })).toBeVisible();
    await expect(this.page.locator('#layout-topbar .breadcrumbs')).toHaveCount(0);
    const firstChild = this.page.locator('#layout-content > *').first();
    await expect(firstChild).not.toHaveClass(/\bmt-6\b/);
  }
}
