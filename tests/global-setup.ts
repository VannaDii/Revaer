import { execFileSync, spawn, spawnSync } from 'child_process';
import dotenv from 'dotenv';
import fs from 'fs';
import http from 'http';
import https from 'https';
import net from 'net';
import path from 'path';

import { cleanupE2EState } from './support/e2e-cleanup';
import { writeState } from './support/e2e-state';
import { repoRoot } from './support/paths';

type UrlParts = {
  host: string;
  port: number;
};

type ProcessInfo = {
  pid: number;
  logPath: string;
};

const LOCAL_HOSTS = new Set(['localhost', '127.0.0.1', 'host.docker.internal']);

const STOP_PATTERNS = [
  'cargo run -p revaer-app',
  'cargo run -p revaer-ui',
  'trunk serve',
  'target/debug/revaer-app',
  'target/release/revaer-app',
];

const KNOWN_DEV_PROCESS = /revaer-app|revaer-ui|trunk serve|cargo run -p revaer-app|cargo run -p revaer-ui/;

export default async function globalSetup(): Promise<void> {
  const root = repoRoot();
  const testsDir = path.join(root, 'tests');
  dotenv.config({ path: path.join(testsDir, '.env') });

  try {
    const apiBaseUrl = process.env.E2E_API_BASE_URL ?? 'http://localhost:7070';
    const baseUrl = process.env.E2E_BASE_URL ?? 'http://localhost:8080';
    const dbAdminUrl =
      process.env.E2E_DB_ADMIN_URL ??
      process.env.REVAER_TEST_DATABASE_URL ??
      'postgres://revaer:revaer@localhost:5432/postgres';
    const dbPrefix = process.env.E2E_DB_PREFIX ?? 'revaer_e2e';
    const fsRoot = process.env.E2E_FS_ROOT ?? root;

    process.env.E2E_API_BASE_URL = apiBaseUrl;
    process.env.E2E_BASE_URL = baseUrl;
    process.env.E2E_DB_ADMIN_URL = dbAdminUrl;
    process.env.E2E_DB_PREFIX = dbPrefix;
    process.env.E2E_FS_ROOT = fsRoot;

    stopDevServers();
    await requirePortFree(7070);
    await requirePortFree(8080);

    const adminUrl = await resolveAdminUrl(dbAdminUrl);
    const adminHost = urlParts(adminUrl).host;
    if (LOCAL_HOSTS.has(adminHost) && !isTruthy(process.env.E2E_SKIP_DB_START)) {
      const dbStartUrl = withPath(adminUrl, '/revaer');
      runCommand('just', ['db-start'], {
        cwd: root,
        env: { ...process.env, DATABASE_URL: dbStartUrl },
      });
    }
    runCommand('just', ['sqlx-install'], { cwd: root });

    const apiBin = path.join(root, 'target', 'debug', 'revaer-app');
    runCommand('cargo', ['build', '-p', 'revaer-app'], { cwd: root });
    if (!fs.existsSync(apiBin)) {
      throw new Error(`revaer-app binary not found at ${apiBin}`);
    }

    const logDir = path.join(testsDir, 'logs');
    fs.mkdirSync(logDir, { recursive: true });

    const activeDbUrl = await createTempDb(adminUrl, dbPrefix, root);
    const apiProcess = spawnLogged(apiBin, [], path.join(logDir, 'api.log'), {
      cwd: root,
      env: { ...process.env, DATABASE_URL: activeDbUrl },
    });
    writeState({ apiPid: apiProcess.pid, dbUrl: activeDbUrl });

    assertApiDb(apiProcess.pid, activeDbUrl);
    await waitForHttp(`${apiBaseUrl}/health`, 80);
    assertApiListener(apiProcess.pid, 7070);

    runCommand('just', ['sync-assets'], { cwd: root });
    runCommand('rustup', ['target', 'add', 'wasm32-unknown-unknown'], { cwd: root });
    if (!commandExists('trunk')) {
      runCommand('cargo', ['install', 'trunk'], { cwd: root });
    }

    const uiProcess = spawnLogged(
      'trunk',
      ['serve', '--dist', 'dist-serve', '--port', '8080'],
      path.join(logDir, 'ui.log'),
      {
        cwd: path.join(root, 'crates', 'revaer-ui'),
        env: {
          ...process.env,
          DATABASE_URL: activeDbUrl,
          RUST_LOG: process.env.RUST_LOG ?? 'info',
          NO_COLOR: 'true',
        },
      },
    );

    await waitForHttp(baseUrl, 80);

    writeState({ apiPid: apiProcess.pid, uiPid: uiProcess.pid, dbUrl: activeDbUrl });
  } catch (error) {
    await cleanupE2EState();
    throw error;
  }
}

function isTruthy(value: string | undefined): boolean {
  if (!value) {
    return false;
  }
  return ['1', 'true', 'yes', 'on'].includes(value.toLowerCase());
}

function runCommand(
  command: string,
  args: string[],
  options?: { cwd?: string; env?: NodeJS.ProcessEnv },
): void {
  execFileSync(command, args, {
    stdio: 'inherit',
    cwd: options?.cwd,
    env: options?.env ?? process.env,
  });
}

function spawnLogged(
  command: string,
  args: string[],
  logPath: string,
  options: { cwd?: string; env?: NodeJS.ProcessEnv },
): ProcessInfo {
  const out = fs.openSync(logPath, 'a');
  const child = spawn(command, args, {
    cwd: options.cwd,
    env: options.env,
    detached: true,
    stdio: ['ignore', out, out],
  });
  if (!child.pid) {
    throw new Error(`Failed to start ${command}.`);
  }
  child.unref();
  return { pid: child.pid, logPath };
}

function commandExists(command: string): boolean {
  const result = spawnSync('sh', ['-c', `command -v ${command}`], {
    stdio: 'ignore',
  });
  return result.status === 0;
}

function urlParts(input: string): UrlParts {
  const parsed = new URL(input);
  return {
    host: parsed.hostname,
    port: parsed.port ? Number(parsed.port) : 5432,
  };
}

function withPath(input: string, pathname: string): string {
  const parsed = new URL(input);
  parsed.pathname = pathname.startsWith('/') ? pathname : `/${pathname}`;
  return parsed.toString();
}

async function resolveAdminUrl(initial: string): Promise<string> {
  if (await dbUrlReachable(initial)) {
    return initial;
  }
  const fallbackUrls = [
    process.env.REVAER_TEST_DATABASE_URL,
    process.env.DATABASE_URL,
  ].filter(Boolean) as string[];
  for (const candidate of fallbackUrls) {
    if (await dbUrlReachable(candidate)) {
      return candidate;
    }
  }
  return initial;
}

async function dbUrlReachable(url: string): Promise<boolean> {
  const { host, port } = urlParts(url);
  return canConnect(host, port, 1000);
}

async function canConnect(host: string, port: number, timeoutMs: number): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = net.createConnection({ host, port });
    const onDone = (result: boolean) => {
      socket.removeAllListeners();
      socket.destroy();
      resolve(result);
    };
    socket.setTimeout(timeoutMs, () => onDone(false));
    socket.once('error', () => onDone(false));
    socket.once('connect', () => onDone(true));
  });
}

async function isPortOpen(port: number): Promise<boolean> {
  return canConnect('127.0.0.1', port, 200);
}

function stopDevServers(): void {
  if (!commandExists('pgrep')) {
    return;
  }
  for (const pattern of STOP_PATTERNS) {
    let output = '';
    try {
      output = execFileSync('pgrep', ['-f', pattern], { encoding: 'utf-8' }).trim();
    } catch {
      continue;
    }
    if (!output) {
      continue;
    }
    for (const pidStr of output.split(/\s+/)) {
      const pid = Number(pidStr);
      if (!pid || pid === process.pid) {
        continue;
      }
      const cmd = readCommand(pid);
      if (!cmd || cmd.includes('pgrep -f') || cmd.includes('ps -p')) {
        continue;
      }
      if (cmd.includes('global-setup')) {
        continue;
      }
      if (cmd) {
        console.error(`Stopping existing Revaer dev process (pid ${pid}: ${cmd})`);
      }
      try {
        process.kill(pid, 'SIGTERM');
      } catch {
        // Ignore missing process.
      }
    }
  }
}

async function requirePortFree(port: number): Promise<void> {
  if (!(await isPortOpen(port))) {
    return;
  }
  const freed = await stopKnownDevProcesses(port);
  if (freed) {
    return;
  }
  throw new Error(`Port ${port} is in use; stop existing services before running ui-e2e.`);
}

async function stopKnownDevProcesses(port: number): Promise<boolean> {
  if (!commandExists('lsof')) {
    return false;
  }
  const pids = pidsOnPort(port);
  if (pids.length === 0) {
    return false;
  }

  let stopped = false;
  for (const pid of pids) {
    const cmd = readCommand(pid);
    if (!cmd) {
      continue;
    }
    if (KNOWN_DEV_PROCESS.test(cmd)) {
      console.error(`Stopping existing Revaer dev process on port ${port} (pid ${pid}: ${cmd})`);
      try {
        process.kill(pid, 'SIGTERM');
      } catch {
        continue;
      }
      stopped = true;
    } else {
      throw new Error(`Port ${port} is in use by a non-Revaer process: ${cmd}`);
    }
  }

  if (stopped) {
    for (let attempt = 0; attempt < 20; attempt += 1) {
      if (!(await isPortOpen(port))) {
        return true;
      }
      await delay(250);
    }
  }
  return false;
}

function pidsOnPort(port: number): number[] {
  try {
    const output = execFileSync('lsof', ['-ti', `:${port}`], {
      encoding: 'utf-8',
    }).trim();
    if (!output) {
      return [];
    }
    return output
      .split(/\s+/)
      .map((pid) => Number(pid))
      .filter((pid) => Number.isFinite(pid) && pid > 0);
  } catch {
    return [];
  }
}

function readCommand(pid: number): string {
  if (!commandExists('ps')) {
    return '';
  }
  try {
    return execFileSync('ps', ['-p', String(pid), '-o', 'args='], {
      encoding: 'utf-8',
    }).trim();
  } catch {
    return '';
  }
}

async function waitForHttp(url: string, attempts: number): Promise<void> {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    if (await httpReady(url)) {
      return;
    }
    await delay(500);
  }
  throw new Error(`Timed out waiting for ${url}`);
}

async function httpReady(url: string): Promise<boolean> {
  return new Promise((resolve) => {
    const target = new URL(url);
    const client = target.protocol === 'https:' ? https : http;
    const request = client.request(
      {
        protocol: target.protocol,
        hostname: target.hostname,
        port: target.port,
        path: `${target.pathname}${target.search}`,
        method: 'GET',
      },
      (response) => {
        response.resume();
        resolve((response.statusCode ?? 500) < 400);
      },
    );
    request.on('error', () => resolve(false));
    request.setTimeout(1000, () => {
      request.destroy();
      resolve(false);
    });
    request.end();
  });
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function createTempDb(adminUrl: string, prefix: string, root: string): Promise<string> {
  const runId = `${Date.now()}_${Math.floor(Math.random() * 10000)}`;
  const dbName = `${prefix}_${runId}`;
  const dbUrl = withPath(adminUrl, dbName);
  runCommand(
    'sqlx',
    ['database', 'create', '--database-url', dbUrl],
    { cwd: root, env: { ...process.env, DATABASE_URL: dbUrl } },
  );
  runCommand(
    'sqlx',
    ['migrate', 'run', '--database-url', dbUrl, '--source', 'crates/revaer-data/migrations'],
    { cwd: root, env: { ...process.env, DATABASE_URL: dbUrl } },
  );
  return dbUrl;
}

function assertApiDb(pid: number, expected: string): void {
  const environPath = `/proc/${pid}/environ`;
  if (!fs.existsSync(environPath)) {
    return;
  }
  const raw = fs.readFileSync(environPath);
  const envLines = raw.toString('utf-8').split('\0');
  const entry = envLines.find((line) => line.startsWith('DATABASE_URL='));
  if (!entry) {
    return;
  }
  const actual = entry.replace('DATABASE_URL=', '');
  if (actual && actual !== expected) {
    throw new Error(`API process started with unexpected DATABASE_URL: ${actual}`);
  }
}

function assertApiListener(pid: number, port: number): void {
  if (!commandExists('lsof')) {
    return;
  }
  try {
    const listener = execFileSync('lsof', ['-tiTCP:' + port, '-sTCP:LISTEN'], {
      encoding: 'utf-8',
    })
      .trim()
      .split(/\s+/)[0];
    if (listener && Number(listener) !== pid) {
      throw new Error(`Port ${port} is already bound by pid ${listener}; expected ${pid}.`);
    }
  } catch {
    // Ignore missing listeners.
  }
}
