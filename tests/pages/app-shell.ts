import { expect, Page } from '@playwright/test';
import { recordUiRoute } from '../support/ui-coverage';

export class AppShell {
  constructor(private readonly page: Page) {}

  async goto(path = '/'): Promise<void> {
    recordUiRoute(path);
    await this.page.goto(path, { waitUntil: 'domcontentloaded' });
    await this.handleOverlays();
    await this.expectShellVisible();
  }

  async expectShellVisible(): Promise<void> {
    await expect(this.page.getByRole('navigation', { name: 'Navbar' })).toBeVisible();
  }

  async navigate(label: string): Promise<void> {
    await this.page.locator('#layout-sidebar').getByRole('link', { name: label }).click();
    const routeMap: Record<string, string> = {
      Dashboard: '/',
      Torrents: '/torrents',
      Settings: '/settings',
      Logs: '/logs',
      Health: '/health',
    };
    recordUiRoute(routeMap[label] ?? '/not-found');
  }

  private async handleOverlays(): Promise<void> {
    const setupOverlay = this.page.locator('.setup-overlay');
    if (await setupOverlay.isVisible()) {
      throw new Error('Setup required; the UI must be activated before running E2E tests.');
    }

    const authOverlay = this.page.locator('.auth-overlay');
    if (await authOverlay.isVisible()) {
      const dismiss = authOverlay.locator('button.btn-circle[aria-label="Dismiss"]');
      if (await dismiss.isVisible()) {
        await dismiss.click();
        await expect(authOverlay).toBeHidden();
        return;
      }
      const fallback = authOverlay.locator('button.btn-ghost.btn-sm', { hasText: 'Dismiss' });
      if (await fallback.isVisible()) {
        await fallback.click();
        await expect(authOverlay).toBeHidden();
      }
    }
  }
}
