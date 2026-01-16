import { request } from '@playwright/test';
import dotenv from 'dotenv';
import fs from 'fs';
import path from 'path';
import { saveSession, type AuthMode } from './support/session';
import { setupHeaders } from './support/headers';
import { resolveFsRoot } from './support/paths';

dotenv.config({ path: path.resolve(__dirname, '.env') });

const VALID_AUTH_MODES = new Set<AuthMode>(['api_key', 'none']);

async function waitForHealthy(apiBaseUrl: string): Promise<void> {
  const api = await request.newContext({ baseURL: apiBaseUrl });
  try {
    for (let attempt = 0; attempt < 30; attempt += 1) {
      const response = await api.get('/health');
      if (response.ok()) {
        return;
      }
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
  } finally {
    await api.dispose();
  }
  throw new Error(`API did not become healthy at ${apiBaseUrl}`);
}

export default async function globalSetup(): Promise<void> {
  const authModeRaw = process.env.E2E_AUTH_MODE;
  if (!authModeRaw) {
    return;
  }
  if (!VALID_AUTH_MODES.has(authModeRaw as AuthMode)) {
    throw new Error(`Unsupported E2E_AUTH_MODE: ${authModeRaw}`);
  }

  const authMode = authModeRaw as AuthMode;
  const apiBaseUrl = process.env.E2E_API_BASE_URL ?? 'http://localhost:7070';
  fs.mkdirSync(resolveFsRoot(), { recursive: true });

  await waitForHealthy(apiBaseUrl);

  const api = await request.newContext({ baseURL: apiBaseUrl });
  try {
    const health = await api.get('/health');
    if (!health.ok()) {
      const body = await health.text();
      throw new Error(`Health check failed: ${health.status()} ${body}`);
    }
    const healthBody = (await health.json()) as { mode?: string };
    if (healthBody.mode !== 'setup') {
      throw new Error(
        `Expected setup mode for E2E bootstrap, got ${healthBody.mode ?? 'unknown'}.`,
      );
    }

    const setupStart = await api.post('/admin/setup/start', { data: {} });
    if (!setupStart.ok()) {
      const body = await setupStart.text();
      throw new Error(`Setup start failed: ${setupStart.status()} ${body}`);
    }
    const setupStartBody = (await setupStart.json()) as { token: string };
    const token = setupStartBody.token;
    if (!token) {
      throw new Error('Setup start did not return a token.');
    }

    let changeset: Record<string, unknown> = {};
    if (authMode === 'none') {
      const snapshotResponse = await api.get('/.well-known/revaer.json');
      if (!snapshotResponse.ok()) {
        const body = await snapshotResponse.text();
        throw new Error(`Snapshot fetch failed: ${snapshotResponse.status()} ${body}`);
      }
      const snapshotBody = (await snapshotResponse.json()) as {
        app_profile?: Record<string, unknown>;
      };
      if (!snapshotBody.app_profile) {
        throw new Error('Snapshot missing app_profile for setup changeset.');
      }
      const appProfile = { ...snapshotBody.app_profile, auth_mode: 'none' };
      changeset = { app_profile: appProfile };
    }

    const setupComplete = await api.post('/admin/setup/complete', {
      data: changeset,
      headers: setupHeaders(token),
    });
    if (!setupComplete.ok()) {
      const body = await setupComplete.text();
      throw new Error(`Setup complete failed: ${setupComplete.status()} ${body}`);
    }
    const setupCompleteBody = (await setupComplete.json()) as { api_key?: string | null };
    const apiKey = setupCompleteBody.api_key ?? undefined;

    saveSession({ authMode, apiKey });
  } finally {
    await api.dispose();
  }
}
