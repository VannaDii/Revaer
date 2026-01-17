import { authHeaders, setupHeaders } from '../headers';
import type { ApiSession, AuthMode } from '../session';
import { createApiClient } from './client';

type SetupOptions = {
  baseUrl: string;
  authMode: AuthMode;
};

type ResetOptions = {
  baseUrl: string;
  session: ApiSession;
};

type HealthResponse = {
  mode?: string;
};

type WellKnownSnapshot = {
  app_profile?: Record<string, unknown>;
};

function assertHealthy(response: Response, body?: HealthResponse): void {
  if (!response.ok) {
    throw new Error(`Health check failed with ${response.status}.`);
  }
  if (body?.mode !== 'setup') {
    throw new Error(
      `Expected setup mode before configuring auth; got ${body?.mode ?? 'unknown'}.`,
    );
  }
}

export async function configureAuthMode(options: SetupOptions): Promise<ApiSession> {
  const publicClient = createApiClient({ baseUrl: options.baseUrl });

  const health = await publicClient.GET('/health');
  assertHealthy(health.response, health.data as HealthResponse | undefined);

  const setupStart = await publicClient.POST('/admin/setup/start', { body: {} });
  if (!setupStart.response.ok || !setupStart.data?.token) {
    throw new Error(`Setup start failed with ${setupStart.response.status}.`);
  }

  let changeset: Record<string, unknown> = {};
  if (options.authMode === 'none') {
    const snapshot = await publicClient.GET('/.well-known/revaer.json');
    if (!snapshot.response.ok) {
      throw new Error(`Snapshot fetch failed with ${snapshot.response.status}.`);
    }
    const appProfile = (snapshot.data as WellKnownSnapshot | undefined)?.app_profile;
    if (!appProfile) {
      throw new Error('Snapshot missing app_profile for setup changeset.');
    }
    changeset = { app_profile: { ...appProfile, auth_mode: 'none' } };
  }

  const setupComplete = await publicClient.POST('/admin/setup/complete', {
    body: changeset,
    headers: setupHeaders(setupStart.data.token),
  });
  if (!setupComplete.response.ok) {
    throw new Error(`Setup complete failed with ${setupComplete.response.status}.`);
  }

  const apiKey = setupComplete.data?.api_key ?? undefined;
  if (options.authMode === 'api_key' && !apiKey) {
    throw new Error('Setup complete did not return an API key.');
  }
  return { authMode: options.authMode, apiKey };
}

export async function factoryReset(options: ResetOptions): Promise<void> {
  const headers = options.session.apiKey ? authHeaders(options.session) : undefined;
  const client = createApiClient({ baseUrl: options.baseUrl, headers });
  const response = await client.POST('/admin/factory-reset', {
    body: { confirm: 'factory reset' },
  });
  if (response.response.status !== 204) {
    throw new Error(`Factory reset failed with ${response.response.status}.`);
  }
}
