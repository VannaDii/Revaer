import { request } from '@playwright/test';
import dotenv from 'dotenv';
import path from 'path';
import { clearSession, loadSession } from './support/session';
import { authHeaders } from './support/headers';

dotenv.config({ path: path.resolve(__dirname, '.env') });

export default async function globalTeardown(): Promise<void> {
  if (!process.env.E2E_AUTH_MODE) {
    return;
  }
  const apiBaseUrl = process.env.E2E_API_BASE_URL ?? 'http://localhost:7070';
  const session = loadSession();
  const api = await request.newContext({ baseURL: apiBaseUrl });
  try {
    const response = await api.post('/admin/factory-reset', {
      data: { confirm: 'factory reset' },
      headers: authHeaders(session),
    });
    if (!response.ok()) {
      const body = await response.text();
      throw new Error(`Factory reset failed: ${response.status()} ${body}`);
    }
  } finally {
    await api.dispose();
    clearSession();
  }
}
