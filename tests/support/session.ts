import fs from 'fs';
import path from 'path';

export type AuthMode = 'api_key' | 'none';

export type ApiSession = {
  authMode: AuthMode;
  apiKey?: string;
};

const defaultSessionPath = path.resolve(__dirname, '..', '.auth', 'session.json');

export function sessionPath(): string {
  const override = process.env.E2E_SESSION_PATH;
  if (!override) {
    return defaultSessionPath;
  }
  return path.isAbsolute(override)
    ? override
    : path.resolve(__dirname, '..', override);
}

export function saveSession(session: ApiSession): void {
  const filePath = sessionPath();
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, JSON.stringify(session, null, 2), 'utf-8');
}

export function loadSession(): ApiSession {
  const filePath = sessionPath();
  if (!fs.existsSync(filePath)) {
    throw new Error(
      `Missing E2E session file at ${filePath}. Ensure global setup ran and E2E_AUTH_MODE is set.`,
    );
  }
  const raw = fs.readFileSync(filePath, 'utf-8');
  const parsed = JSON.parse(raw) as ApiSession;
  if (!parsed.authMode) {
    throw new Error(`E2E session file missing authMode at ${filePath}.`);
  }
  return parsed;
}

export function clearSession(): void {
  const filePath = sessionPath();
  if (fs.existsSync(filePath)) {
    fs.unlinkSync(filePath);
  }
}
