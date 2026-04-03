import { expect, Page } from '@playwright/test';
import { recordUiRoute } from '../support/ui-coverage';
import { dismissBlockingOverlays } from './overlays';

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
    await this.handleOverlays();
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
    await dismissBlockingOverlays(this.page);
  }
}
