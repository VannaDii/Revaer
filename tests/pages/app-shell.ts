import { expect, Locator, Page } from '@playwright/test';
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
    if (await setupOverlay.isVisible().catch(() => false)) {
      throw new Error('Setup required; the UI must be activated before running E2E tests.');
    }

    const authOverlay = this.page.locator('.auth-overlay');
    if (await authOverlay.isVisible().catch(() => false)) {
      const useAnonymous = authOverlay.getByRole('button', { name: /Use anonymous/i }).first();
      if (await this.clickOverlayControl(useAnonymous)) {
        await expect(authOverlay).toBeHidden();
        return;
      }

      const dismissIcon = authOverlay.getByLabel('Dismiss').first();
      if (await this.clickOverlayControl(dismissIcon)) {
        await expect(authOverlay).toBeHidden();
        return;
      }

      const dismissText = authOverlay.getByRole('button', { name: 'Dismiss' }).first();
      if (await this.clickOverlayControl(dismissText)) {
        await expect(authOverlay).toBeHidden();
      }
    }
  }

  private async clickOverlayControl(control: Locator): Promise<boolean> {
    for (let attempt = 0; attempt < 3; attempt += 1) {
      if (!(await control.isVisible().catch(() => false))) {
        return false;
      }
      try {
        await control.click({ force: true, timeout: 2_000 });
        return true;
      } catch {
        continue;
      }
    }
    return false;
  }
}
