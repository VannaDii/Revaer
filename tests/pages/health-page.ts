import { expect, Page } from '@playwright/test';

export class HealthPage {
  constructor(private readonly page: Page) {}

  async expectLoaded(): Promise<void> {
    await expect(this.page.getByText('System health', { exact: true })).toBeVisible();
    await expect(this.page.getByText('Metrics', { exact: true })).toBeVisible();
    await expect(this.page.getByText('Basic', { exact: true })).toBeVisible();
    await expect(this.page.getByText('Full', { exact: true })).toBeVisible();
  }
}
