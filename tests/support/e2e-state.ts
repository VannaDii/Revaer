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

const stateFile = path.join(repoRoot(), 'tests', 'test-results', 'e2e-state.json');

export function statePath(): string {
  return stateFile;
}

export function writeState(state: E2EState): void {
  const dir = path.dirname(stateFile);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(stateFile, JSON.stringify(state, null, 2));
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
    return JSON.parse(raw) as E2EState;
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
