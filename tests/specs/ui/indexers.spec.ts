import { test, expect } from '../../fixtures/app';

test.describe('Indexers', () => {
  test('renders the admin console route', async ({ app, page }) => {
    await app.goto('/indexers');

    await expect(page.getByRole('heading', { name: 'Indexers' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Catalog' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Cardigann import' })).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Health notifications' }),
    ).toBeVisible();
    await expect(page.getByRole('button', { name: 'Refresh definitions' })).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Import Cardigann YAML' }),
    ).toBeVisible();
    await expect(page.getByText('RSS enabled', { exact: true })).toBeVisible();
    await expect(
      page.getByText('Automatic search enabled', { exact: true }),
    ).toBeVisible();
    await expect(page.getByRole('heading', { name: 'RSS management' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Category overrides' })).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Connectivity & reputation' }),
    ).toBeVisible();
    await expect(page.getByRole('button', { name: 'Fetch RSS status' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Upsert tracker mapping' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Mark RSS item seen' })).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Fetch connectivity profile' }),
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Fetch routing policy' }),
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Fetch source reputation' }),
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Fetch health events' }),
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Fetch notification hooks' }),
    ).toBeVisible();
    await expect(page.getByRole('heading', { name: 'App sync', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Provision app sync' })).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Connectivity profile' }),
    ).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Source reputation' }),
    ).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Health events' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Import status' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Import results' })).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Source conflict resolution' }),
    ).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Backup & restore' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Activity log' })).toBeVisible();
  });
});
