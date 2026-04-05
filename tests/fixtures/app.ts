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
      type StorageSeeds = {
        local: Map<string, string>;
        session: Map<string, string>;
      };

      type SeededWindow = Window & {
        __revaerE2eStoragePatch?: boolean;
        __revaerE2eStorageSeeds?: StorageSeeds;
      };

      const seededWindow = window as SeededWindow;
      seededWindow.__revaerE2eStorageSeeds = {
        local: new Map<string, string>(),
        session: new Map<string, string>(),
      };

      const storageSeeds = seededWindow.__revaerE2eStorageSeeds;
      const setSeed = (key: string, value: unknown): void => {
        storageSeeds.local.set(key, JSON.stringify(value));
      };

      if (!seededWindow.__revaerE2eStoragePatch) {
        const storageProto = Object.getPrototypeOf(window.localStorage) as Storage;
        const originalGetItem = storageProto.getItem;
        const originalSetItem = storageProto.setItem;
        const originalRemoveItem = storageProto.removeItem;
        const originalClear = storageProto.clear;
        const activeSeedMap = (storage: Storage): Map<string, string> => {
          const nextSeeds = seededWindow.__revaerE2eStorageSeeds;
          if (!nextSeeds) {
            return new Map<string, string>();
          }
          return storage === window.sessionStorage
            ? nextSeeds.session
            : nextSeeds.local;
        };

        storageProto.getItem = function getItem(key: string): string | null {
          const seededValue = activeSeedMap(this).get(key);
          if (seededValue !== undefined) {
            return seededValue;
          }
          return originalGetItem.call(this, key);
        };

        storageProto.setItem = function setItem(key: string, value: string): void {
          activeSeedMap(this).delete(key);
          originalSetItem.call(this, key, value);
        };

        storageProto.removeItem = function removeItem(key: string): void {
          activeSeedMap(this).delete(key);
          originalRemoveItem.call(this, key);
        };

        storageProto.clear = function clear(): void {
          const seedMap = activeSeedMap(this);
          seedMap.clear();
          originalClear.call(this);
        };

        seededWindow.__revaerE2eStoragePatch = true;
      }

      if (session.authMode === 'api_key' && session.apiKey) {
        setSeed('revaer.auth.mode', 'api_key');
        setSeed('revaer.api_key', session.apiKey);
        setSeed('revaer.api_key_expires_at', Date.now() + 86_400_000);
        return;
      }

      setSeed('revaer.auth.mode', 'api_key');
      setSeed('revaer.auth.anonymous', true);
    }, apiSession);
    await use(new AppShell(page));
  },
});

export { expect };
