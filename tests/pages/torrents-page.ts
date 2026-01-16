import { expect, Page } from '@playwright/test';

export class TorrentsPage {
  constructor(private readonly page: Page) {}

  async expectLoaded(): Promise<void> {
    await expect(this.page.getByLabel('Search torrents')).toBeVisible();
    await expect(this.page.getByRole('button', { name: 'Add' })).toBeVisible();
    await expect(this.page.getByRole('button', { name: 'Create torrent' })).toBeVisible();
    await expect(this.page.getByRole('columnheader', { name: 'Name' })).toBeVisible();
  }

  async openAddModal(): Promise<void> {
    await this.page.getByRole('button', { name: 'Add' }).click();
    await expect(this.page.getByRole('heading', { name: 'Add torrent' })).toBeVisible();
  }

  async openCreateModal(): Promise<void> {
    await this.page.getByRole('button', { name: 'Create torrent' }).click();
    await expect(this.page.getByRole('heading', { name: 'Create torrent' })).toBeVisible();
  }

  async closeModal(): Promise<void> {
    const closeButton = this.page.getByRole('button', { name: 'Close modal' });
    await closeButton.click();
    await expect(closeButton).toBeHidden();
  }
}
