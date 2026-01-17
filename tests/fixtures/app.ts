import { test as base, expect } from '@playwright/test';
import path from 'path';
import { AppShell } from '../pages/app-shell';
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
    await use(new AppShell(page));
  },
});

export { expect };
