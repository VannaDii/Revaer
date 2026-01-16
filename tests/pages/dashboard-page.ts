import { expect, Page } from '@playwright/test';

export class DashboardPage {
  constructor(private readonly page: Page) {}

  async expectLoaded(): Promise<void> {
    await expect(this.page.getByText('Business Overview', { exact: true })).toBeVisible();
    await expect(this.page.getByText('Storage Status', { exact: true })).toBeVisible();
    await expect(this.page.getByText('Tracker Health', { exact: true })).toBeVisible();
  }
}
