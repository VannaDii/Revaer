import { test as base, expect } from '@playwright/test';
import { AppShell } from '../pages/app-shell';

type AppFixtures = {
  app: AppShell;
};

export const test = base.extend<AppFixtures>({
  app: async ({ page }, use) => {
    await use(new AppShell(page));
  },
});

export { expect };
