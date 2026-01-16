import { test, expect } from '@playwright/test';


test.describe('Public API', () => {
  test('health endpoints respond', async ({ request }) => {
    const health = await request.get('/health');
    expect(health.ok()).toBeTruthy();

    const healthBody = (await health.json()) as { status?: string; mode?: string };
    expect(healthBody.status).toBeTruthy();
    expect(healthBody.mode).toBeTruthy();

    const full = await request.get('/health/full');
    expect(full.ok()).toBeTruthy();

    const fullBody = (await full.json()) as { status?: string; mode?: string; revision?: number };
    expect(fullBody.status).toBeTruthy();
    expect(fullBody.mode).toBeTruthy();
    expect(fullBody.revision).toBeDefined();
  });

  test('well-known snapshot is reachable', async ({ request }) => {
    const response = await request.get('/.well-known/revaer.json');
    expect(response.ok()).toBeTruthy();

    const body = (await response.json()) as {
      app_profile?: { mode?: string; auth_mode?: string };
    };
    expect(body.app_profile?.mode).toBeTruthy();
    expect(body.app_profile?.auth_mode).toBeTruthy();
  });

  test('metrics endpoint responds', async ({ request }) => {
    const response = await request.get('/metrics');
    expect(response.ok()).toBeTruthy();

    const text = await response.text();
    expect(text.trim().length).toBeGreaterThan(0);
  });

  test('openapi document is reachable', async ({ request }) => {
    const response = await request.get('/docs/openapi.json');
    expect(response.ok()).toBeTruthy();

    const body = (await response.json()) as { openapi?: string; paths?: unknown };
    expect(body.openapi).toBeTruthy();
    expect(body.paths).toBeTruthy();
  });
});
