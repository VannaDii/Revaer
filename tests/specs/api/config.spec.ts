import { test, expect } from '@playwright/test';
import { authHeaders } from '../../support/headers';
import { loadSession, type ApiSession } from '../../support/session';

let session: ApiSession;

test.beforeAll(() => {
  session = loadSession();
});

test.describe('Config and dashboard', () => {
  test('fetches dashboard snapshot', async ({ request }) => {
    const response = await request.get('/v1/dashboard', {
      headers: authHeaders(session),
    });
    expect(response.ok()).toBeTruthy();

    const body = (await response.json()) as {
      download_bps?: number;
      upload_bps?: number;
    };
    expect(body.download_bps).toBeDefined();
    expect(body.upload_bps).toBeDefined();
  });

  test('gets and patches config snapshot', async ({ request }) => {
    const snapshot = await request.get('/v1/config', {
      headers: authHeaders(session),
    });
    expect(snapshot.ok()).toBeTruthy();

    const patch = await request.patch('/v1/config', {
      data: {},
      headers: authHeaders(session),
    });
    expect(patch.ok()).toBeTruthy();

    const body = (await patch.json()) as { revision?: number };
    expect(body.revision).toBeDefined();
  });

  test('applies admin settings patch', async ({ request }) => {
    const response = await request.patch('/admin/settings', {
      data: {},
      headers: authHeaders(session),
    });
    expect(response.ok()).toBeTruthy();

    const body = (await response.json()) as { revision?: number };
    expect(body.revision).toBeDefined();
  });
});
