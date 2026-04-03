import { expect, Page } from '@playwright/test';

export async function dismissBlockingOverlays(page: Page): Promise<void> {
  const setupOverlay = page.locator('.setup-overlay');
  if (await setupOverlay.isVisible()) {
    throw new Error('Setup required; the UI must be activated before running E2E tests.');
  }

  const authOverlay = page.locator('.auth-overlay');
  if (!(await authOverlay.isVisible())) {
    return;
  }

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
