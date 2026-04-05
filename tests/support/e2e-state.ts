import crypto from 'crypto';
import fs from 'fs';
import path from 'path';

import { repoRoot } from './paths';
import type { ApiSession } from './session';

export type E2EState = {
  apiPid?: number;
  uiPid?: number;
  dbUrl?: string;
  apiSession?: ApiSession;
};

const stateFile = path.join(repoRoot(), 'tests', '.runtime', 'e2e-state.json');
const secretEnvVar = 'REVAER_E2E_STATE_KEY';

type PersistedState = Omit<E2EState, 'apiSession'> & {
  apiSessionEncrypted?: string;
};

export function writeState(state: E2EState): void {
  const dir = path.dirname(stateFile);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(stateFile, JSON.stringify(serializeState(state), null, 2));
  fs.chmodSync(stateFile, 0o600);
}

export function mergeState(state: Partial<E2EState>): void {
  const current = readState() ?? {};
  writeState({ ...current, ...state });
}

export function readState(): E2EState | null {
  if (!fs.existsSync(stateFile)) {
    return null;
  }
  const raw = fs.readFileSync(stateFile, 'utf-8');
  try {
    return deserializeState(JSON.parse(raw) as PersistedState);
  } catch {
    return null;
  }
}

export function clearState(): void {
  if (!fs.existsSync(stateFile)) {
    return;
  }
  fs.unlinkSync(stateFile);
}

function serializeState(state: E2EState): PersistedState {
  const { apiSession, ...rest } = state;
  if (!apiSession) {
    return rest;
  }
  return {
    ...rest,
    apiSessionEncrypted: encryptSession(apiSession),
  };
}

function deserializeState(state: PersistedState): E2EState {
  const { apiSessionEncrypted, ...rest } = state;
  if (!apiSessionEncrypted) {
    return rest;
  }
  return {
    ...rest,
    apiSession: decryptSession(apiSessionEncrypted),
  };
}

function encryptSession(session: ApiSession): string {
  const key = encryptionKey();
  const iv = crypto.randomBytes(12);
  const cipher = crypto.createCipheriv('aes-256-gcm', key, iv);
  const plaintext = Buffer.from(JSON.stringify(session), 'utf-8');
  const ciphertext = Buffer.concat([cipher.update(plaintext), cipher.final()]);
  const tag = cipher.getAuthTag();
  return Buffer.concat([iv, tag, ciphertext]).toString('base64');
}

function decryptSession(payload: string): ApiSession {
  const key = encryptionKey();
  const data = Buffer.from(payload, 'base64');
  const iv = data.subarray(0, 12);
  const tag = data.subarray(12, 28);
  const ciphertext = data.subarray(28);
  const decipher = crypto.createDecipheriv('aes-256-gcm', key, iv);
  decipher.setAuthTag(tag);
  const plaintext = Buffer.concat([decipher.update(ciphertext), decipher.final()]);
  return JSON.parse(plaintext.toString('utf-8')) as ApiSession;
}

function encryptionKey(): Buffer {
  const raw = process.env[secretEnvVar];
  if (!raw) {
    throw new Error(`Missing ${secretEnvVar}.`);
  }
  return Buffer.from(raw, 'hex');
}
