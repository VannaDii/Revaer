import { test, expect } from '@playwright/test';
import { authHeaders } from '../../support/headers';
import { loadSession, type ApiSession } from '../../support/session';

let session: ApiSession;

test.beforeAll(() => {
  session = loadSession();
});

test.describe('Auth', () => {
  test('refresh endpoint matches auth mode', async ({ request }) => {
    const response = await request.post('/v1/auth/refresh', {
      headers: authHeaders(session),
    });

    if (session.authMode === 'api_key') {
      expect(response.ok()).toBeTruthy();
      const body = (await response.json()) as { api_key_expires_at?: string };
      expect(body.api_key_expires_at).toBeTruthy();
    } else {
      expect(response.status()).toBe(401);
    }
  });

  test('protected endpoints respect API key requirements', async ({ request }) => {
    const response = await request.get('/v1/config');
    if (session.authMode === 'api_key') {
      expect(response.status()).toBe(401);
    } else {
      expect(response.ok()).toBeTruthy();
    }
  });
});
