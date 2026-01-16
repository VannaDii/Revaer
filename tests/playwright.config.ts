import { defineConfig } from '@playwright/test';
import dotenv from 'dotenv';
import path from 'path';

dotenv.config({ path: path.resolve(__dirname, '.env') });

type BrowserName = 'chromium' | 'firefox' | 'webkit';

type VideoMode = 'on' | 'off' | 'retain-on-failure' | 'on-first-retry';
type ScreenshotMode = 'on' | 'off' | 'only-on-failure';
type TraceMode = 'on' | 'off' | 'retain-on-failure' | 'on-first-retry';

const baseURL = process.env.E2E_BASE_URL ?? 'http://localhost:8080';
const apiBaseURL = process.env.E2E_API_BASE_URL ?? 'http://localhost:7070';
const headless = parseBoolean(process.env.E2E_HEADLESS, true);
const viewportWidth = parseNumber(process.env.E2E_VIEWPORT_WIDTH, 1440);
const viewportHeight = parseNumber(process.env.E2E_VIEWPORT_HEIGHT, 900);
const retries = parseNumber(process.env.E2E_RETRIES, process.env.CI ? 2 : 0);
const workers = parseOptionalNumber(process.env.E2E_WORKERS);

const testTimeout = parseNumber(process.env.E2E_TEST_TIMEOUT_MS, 30_000);
const expectTimeout = parseNumber(process.env.E2E_EXPECT_TIMEOUT_MS, 5_000);
const actionTimeout = parseNumber(process.env.E2E_ACTION_TIMEOUT_MS, 10_000);
const navigationTimeout = parseNumber(process.env.E2E_NAVIGATION_TIMEOUT_MS, 15_000);

const trace = (process.env.E2E_TRACE as TraceMode | undefined) ?? 'on-first-retry';
const video = (process.env.E2E_VIDEO as VideoMode | undefined) ?? 'retain-on-failure';
const screenshot =
  (process.env.E2E_SCREENSHOT as ScreenshotMode | undefined) ?? 'only-on-failure';

const browsers = parseBrowserList(process.env.E2E_BROWSERS);

export default defineConfig({
  testDir: './specs',
  outputDir: 'test-results',
  timeout: testTimeout,
  fullyParallel: true,
  retries,
  workers,
  globalSetup: './global-setup',
  globalTeardown: './global-teardown',
  expect: {
    timeout: expectTimeout,
  },
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL,
    headless,
    viewport: { width: viewportWidth, height: viewportHeight },
    actionTimeout,
    navigationTimeout,
    trace,
    video,
    screenshot,
  },
  projects: [
    {
      name: 'api',
      testMatch: /api\/.*\.spec\.ts/,
      use: {
        baseURL: apiBaseURL,
      },
      workers: 1,
    },
    ...browsers.map((name) => ({
      name: `ui-${name}`,
      dependencies: ['api'],
      testMatch: /ui\/.*\.spec\.ts/,
      use: { browserName: name },
    })),
  ],
});

function parseBrowserList(value: string | undefined): BrowserName[] {
  if (!value) {
    return ['chromium'];
  }
  const browsers = value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
  const valid = new Set<BrowserName>(['chromium', 'firefox', 'webkit']);
  const results = browsers.filter((name): name is BrowserName =>
    valid.has(name as BrowserName),
  );
  return results.length > 0 ? results : ['chromium'];
}

function parseBoolean(value: string | undefined, fallback: boolean): boolean {
  if (value === undefined) {
    return fallback;
  }
  return ['1', 'true', 'yes', 'on'].includes(value.toLowerCase());
}

function parseNumber(value: string | undefined, fallback: number): number {
  if (!value) {
    return fallback;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function parseOptionalNumber(value: string | undefined): number | undefined {
  if (!value) {
    return undefined;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}
