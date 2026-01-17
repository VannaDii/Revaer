import { test as base, expect, type TestInfo } from '@playwright/test';
import path from 'path';
import { createApiClient, type ApiClient } from '../support/api/client';
import { setApiCoveragePath } from '../support/api/coverage';
import { configureAuthMode, factoryReset } from '../support/api/setup';
import type { ApiSession, AuthMode } from '../support/session';

type ApiFixtures = {
  api: ApiClient;
  publicApi: ApiClient;
  session: ApiSession;
};

type CoverageFixture = {
  _apiCoverage: void;
};

function resolveBaseUrl(testInfo: TestInfo): string {
  const baseUrl = testInfo.project.use.baseURL;
  if (typeof baseUrl === 'string') {
    return baseUrl;
  }
  return process.env.E2E_API_BASE_URL ?? 'http://localhost:7070';
}

function projectAuthMode(testInfo: TestInfo): AuthMode {
  const metadata = testInfo.project.metadata as { authMode?: AuthMode };
  if (!metadata?.authMode) {
    throw new Error(`Missing authMode metadata for ${testInfo.project.name}.`);
  }
  return metadata.authMode;
}

function projectCoveragePath(testInfo: TestInfo): string | undefined {
  const metadata = testInfo.project.metadata as { coverageFile?: string };
  if (!metadata?.coverageFile) {
    return undefined;
  }
  return path.resolve(__dirname, '..', metadata.coverageFile);
}

function projectKeepActive(testInfo: TestInfo): boolean {
  const metadata = testInfo.project.metadata as { keepActive?: boolean };
  return Boolean(metadata?.keepActive);
}

function withWorkerSuffix(filePath: string, workerIndex: number): string {
  const parsed = path.parse(filePath);
  const suffix = `worker-${workerIndex}`;
  const name = parsed.name.endsWith(suffix) ? parsed.name : `${parsed.name}-${suffix}`;
  const ext = parsed.ext || '.json';
  return path.join(parsed.dir, `${name}${ext}`);
}

export const test = base.extend<ApiFixtures & CoverageFixture>({
  _apiCoverage: [
    async ({}, use, testInfo) => {
      const coveragePath = projectCoveragePath(testInfo);
      if (coveragePath) {
        setApiCoveragePath(withWorkerSuffix(coveragePath, testInfo.workerIndex));
      }
      await use();
    },
    { scope: 'worker' },
  ],
  session: [
    async ({ _apiCoverage }, use, testInfo) => {
      const baseUrl = resolveBaseUrl(testInfo);
      const authMode = projectAuthMode(testInfo);
      const session = await configureAuthMode({ baseUrl, authMode });
      try {
        await use(session);
      } finally {
        if (!projectKeepActive(testInfo)) {
          await factoryReset({ baseUrl, session });
        }
      }
    },
    { scope: 'worker' },
  ],
  api: async ({ session }, use, testInfo) => {
    const baseUrl = resolveBaseUrl(testInfo);
    const headers = session.apiKey ? { 'x-revaer-api-key': session.apiKey } : undefined;
    await use(createApiClient({ baseUrl, headers }));
  },
  publicApi: async ({}, use, testInfo) => {
    const baseUrl = resolveBaseUrl(testInfo);
    await use(createApiClient({ baseUrl }));
  },
});

export { expect };
