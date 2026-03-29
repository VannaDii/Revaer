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
    const setupOverlay = await this.page.$('.setup-overlay');
    if (setupOverlay && (await setupOverlay.isVisible())) {
      throw new Error('Setup required; the UI must be activated before running E2E tests.');
    }

    const authOverlay = await this.page.$('.auth-overlay');
    if (authOverlay && (await authOverlay.isVisible())) {
      const useAnonymous = await authOverlay.$('button:has-text("Use anonymous")');
      if (useAnonymous && (await useAnonymous.isVisible())) {
        await useAnonymous.click();
        await expect(this.page.locator('.auth-overlay')).toBeHidden();
        return;
      }

      const dismiss =
        (await authOverlay.$('button.btn-circle[aria-label="Dismiss"]')) ??
        (await authOverlay.$('button.btn-ghost.btn-sm:has-text("Dismiss")'));
      if (dismiss && (await dismiss.isVisible())) {
        await dismiss.click();
        await expect(this.page.locator('.auth-overlay')).toBeHidden();
      }
    }
  }
}
