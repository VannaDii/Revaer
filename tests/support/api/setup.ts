import fs from 'fs';
import path from 'path';
import { readState } from '../e2e-state';
import { authHeaders, setupHeaders } from '../headers';
import { repoRoot } from '../paths';
import type { ApiSession, AuthMode } from '../session';
import { createApiClient, type ApiClient } from './client';

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

type ApiLogTail = {
  logPath: string;
  tail: string;
};

function parseHealthMode(response: Response, body?: HealthResponse): string {
  if (!response.ok) {
    throw new Error(`Health check failed with ${response.status}.`);
  }
  const mode = body?.mode;
  if (!mode) {
    throw new Error('Health check missing mode in response.');
  }
  return mode;
}

function assertSetupMode(mode: string, context: string): void {
  if (mode !== 'setup') {
    throw new Error(`Expected setup mode ${context}; got ${mode}.`);
  }
}

async function ensureSetupMode(client: ApiClient): Promise<void> {
  const health = await fetchHealth(client, 'initial');
  const mode = parseHealthMode(health.response, health.data as HealthResponse | undefined);

  const reset = await client.POST('/admin/factory-reset', {
    body: { confirm: 'factory reset' },
  });
  if (reset.response.status !== 204) {
    throw new Error(
      `Factory reset failed with ${reset.response.status} while in ${mode} mode.`,
    );
  }

  const healthAfter = await fetchHealth(client, 'after factory reset');
  const resetMode = parseHealthMode(
    healthAfter.response,
    healthAfter.data as HealthResponse | undefined,
  );
  assertSetupMode(resetMode, 'after factory reset');
}

export async function configureAuthMode(options: SetupOptions): Promise<ApiSession> {
  const publicClient = createApiClient({ baseUrl: options.baseUrl });

  await ensureSetupMode(publicClient);

  const setupStart = await publicClient.POST('/admin/setup/start', { body: {} });
  if (!setupStart.response.ok || !setupStart.data?.token) {
    throw new Error(`Setup start failed with ${setupStart.response.status}.`);
  }

  const snapshot = await publicClient.GET('/.well-known/revaer.json');
  if (!snapshot.response.ok) {
    throw new Error(`Snapshot fetch failed with ${snapshot.response.status}.`);
  }
  const appProfile = (snapshot.data as WellKnownSnapshot | undefined)?.app_profile;
  if (!appProfile) {
    throw new Error('Snapshot missing app_profile for setup changeset.');
  }
  const changeset: Record<string, unknown> = {
    app_profile: { ...appProfile, auth_mode: options.authMode },
  };

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

function isPidAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    const code = typeof error === 'object' && error ? (error as NodeJS.ErrnoException).code : '';
    return code === 'EPERM';
  }
}

function apiLogTail(lines: number): ApiLogTail | null {
  const logPath = path.join(repoRoot(), 'tests', 'logs', 'api.log');
  if (!fs.existsSync(logPath)) {
    return null;
  }
  const raw = fs.readFileSync(logPath, 'utf-8').trim();
  if (!raw) {
    return null;
  }
  const entries = raw.split(/\r?\n/);
  const tail = entries.slice(-lines).join('\n');
  return { logPath, tail };
}

function apiDiagnostics(): string {
  const state = readState();
  if (!state) {
    return 'E2E state is missing; global setup may not have started the API process.';
  }
  if (!state.apiPid) {
    return 'E2E state is missing the API pid; global setup may not have started the API process.';
  }
  if (!isPidAlive(state.apiPid)) {
    return `API process ${state.apiPid} is not running.`;
  }
  return `API process ${state.apiPid} is running.`;
}

function apiFailureMessage(context: string, error: unknown): string {
  const detail = error instanceof Error ? error.message : String(error);
  const lines = [context, detail, apiDiagnostics()];
  const tail = apiLogTail(60);
  if (tail) {
    lines.push(`Last 60 lines from ${tail.logPath}:`);
    lines.push(tail.tail);
  }
  return lines.filter(Boolean).join('\n');
}

async function fetchHealth(client: ApiClient, context: string): Promise<{
  response: Response;
  data?: HealthResponse;
}> {
  const state = readState();
  if (state?.apiPid && !isPidAlive(state.apiPid)) {
    throw new Error(
      apiFailureMessage(
        `API process ${state.apiPid} exited before ${context} health check.`,
        'API process not running',
      ),
    );
  }
  try {
    return await client.GET('/health');
  } catch (error) {
    throw new Error(apiFailureMessage(`Health check (${context}) failed to reach API.`, error));
  }
}
