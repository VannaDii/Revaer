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

  const actionDismiss = authOverlay.getByRole('button', { name: 'Dismiss' }).last();
  if (await actionDismiss.isVisible()) {
    await actionDismiss.click();
    if (!(await authOverlay.isVisible())) {
      return;
    }
  }

  const iconDismiss = authOverlay.locator('button.btn-circle[aria-label="Dismiss"]');
  if (await iconDismiss.isVisible()) {
    await iconDismiss.click();
  }

  if (await authOverlay.isVisible() && (await actionDismiss.isVisible())) {
    await actionDismiss.click();
  }

  await expect(authOverlay).toBeHidden();
}
