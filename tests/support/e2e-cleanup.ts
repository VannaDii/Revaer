import { execFileSync, spawnSync } from 'child_process';

import { clearState, readState } from './e2e-state';

const LOCAL_HOSTS = new Set(['localhost', '127.0.0.1', 'host.docker.internal']);

export async function cleanupE2EState(): Promise<void> {
  const state = readState();
  if (!state) {
    return;
  }

  await terminateProcess(state.uiPid);
  await terminateProcess(state.apiPid);
  if (state.dbUrl) {
    try {
      dropTempDb(state.dbUrl);
    } catch (error) {
      console.warn('Failed to drop temp database:', error);
    }
  }
  clearState();
}

async function terminateProcess(pid?: number): Promise<void> {
  if (!pid || !isAlive(pid)) {
    return;
  }
  try {
    process.kill(-pid, 'SIGTERM');
  } catch {
    try {
      process.kill(pid, 'SIGTERM');
    } catch {
      return;
    }
  }
  for (let attempt = 0; attempt < 20; attempt += 1) {
    if (!isAlive(pid)) {
      return;
    }
    await delay(250);
  }
  try {
    process.kill(-pid, 'SIGKILL');
  } catch {
    try {
      process.kill(pid, 'SIGKILL');
    } catch {
      // ignore
    }
  }
}

function isAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function dropTempDb(dbUrl: string): void {
  const parsed = new URL(dbUrl);
  if (!LOCAL_HOSTS.has(parsed.hostname)) {
    console.warn(`Refusing to drop non-local database (${parsed.hostname}).`);
    return;
  }
  if (!commandExists('sqlx')) {
    console.warn('sqlx not available; skipping database cleanup.');
    return;
  }
  execFileSync('sqlx', ['database', 'drop', '--database-url', dbUrl, '-y'], {
    env: { ...process.env, DATABASE_URL: dbUrl },
    stdio: 'ignore',
  });
}

function commandExists(command: string): boolean {
  const result = spawnSync('sh', ['-c', `command -v ${command}`], {
    stdio: 'ignore',
  });
  return result.status === 0;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
