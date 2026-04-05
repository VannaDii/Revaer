import { test as base, expect } from '@playwright/test';
import path from 'path';
import { AppShell } from '../pages/app-shell';
import { readState } from '../support/e2e-state';
import { setUiCoveragePath } from '../support/ui-coverage';

type AppFixtures = {
  app: AppShell;
};

type CoverageFixture = {
  _uiCoverage: void;
};

function withWorkerSuffix(filePath: string, workerIndex: number): string {
  const parsed = path.parse(filePath);
  const suffix = `worker-${workerIndex}`;
  const name = parsed.name.endsWith(suffix) ? parsed.name : `${parsed.name}-${suffix}`;
  const ext = parsed.ext || '.json';
  return path.join(parsed.dir, `${name}${ext}`);
}

export const test = base.extend<AppFixtures & CoverageFixture>({
  _uiCoverage: [
    async ({}, use, testInfo) => {
      const metadata = testInfo.project.metadata as { coverageFile?: string };
      if (metadata?.coverageFile) {
        const basePath = path.resolve(__dirname, '..', metadata.coverageFile);
        const coveragePath = withWorkerSuffix(basePath, testInfo.workerIndex);
        setUiCoveragePath(coveragePath);
      }
      await use();
    },
    { scope: 'worker' },
  ],
  app: async ({ page, _uiCoverage }, use) => {
    const apiSession = readState()?.apiSession;
    if (!apiSession) {
      throw new Error('Missing API session in E2E state for UI fixture.');
    }

    await page.addInitScript((session) => {
      const setJsonSessionStorage = (key: string, value: unknown): void => {
        window.sessionStorage.setItem(key, JSON.stringify(value));
      };

      if (session.authMode === 'api_key' && session.apiKey) {
        setJsonSessionStorage('revaer.auth.mode', 'api_key');
        setJsonSessionStorage('revaer.api_key', session.apiKey);
        setJsonSessionStorage(
          'revaer.api_key_expires_at',
          Date.now() + 86_400_000,
        );
        window.localStorage.removeItem('revaer.auth.mode');
        window.localStorage.removeItem('revaer.api_key');
        window.localStorage.removeItem('revaer.api_key_expires_at');
        window.localStorage.removeItem('revaer.auth.anonymous');
        return;
      }

      setJsonSessionStorage('revaer.auth.mode', 'api_key');
      setJsonSessionStorage('revaer.auth.anonymous', true);
      window.sessionStorage.removeItem('revaer.api_key');
      window.sessionStorage.removeItem('revaer.api_key_expires_at');
      window.localStorage.removeItem('revaer.auth.mode');
      window.localStorage.removeItem('revaer.api_key');
      window.localStorage.removeItem('revaer.api_key_expires_at');
      window.localStorage.removeItem('revaer.auth.anonymous');
    }, apiSession);
    await use(new AppShell(page));
  },
});

export { expect };
