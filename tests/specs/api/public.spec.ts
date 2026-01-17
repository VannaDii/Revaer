import { test, expect } from '../../fixtures/api';

test.describe('Public API', () => {
  test('health endpoints respond', async ({ publicApi }) => {
    const health = await publicApi.GET('/health');
    expect(health.response.ok).toBeTruthy();
    expect(health.data?.status).toBeTruthy();
    expect(health.data?.mode).toBeTruthy();

    const full = await publicApi.GET('/health/full');
    expect(full.response.ok).toBeTruthy();
    expect(full.data?.status).toBeTruthy();
    expect(full.data?.mode).toBeTruthy();
    expect(full.data?.revision).toBeDefined();
  });

  test('well-known snapshot is reachable', async ({ publicApi }) => {
    const response = await publicApi.GET('/.well-known/revaer.json');
    expect(response.response.ok).toBeTruthy();
    expect(response.data?.app_profile).toBeTruthy();
  });

  test('metrics endpoint responds', async ({ publicApi }) => {
    const response = await publicApi.GET('/metrics', { parseAs: 'text' });
    expect(response.response.ok).toBeTruthy();

    const payload = response.data ?? '';
    expect(payload.trim().length).toBeGreaterThan(0);
  });

  test('openapi document is reachable', async ({ publicApi }) => {
    const response = await publicApi.GET('/docs/openapi.json');
    expect(response.response.ok).toBeTruthy();
    expect(response.data?.openapi).toBeTruthy();
    expect(response.data?.paths).toBeTruthy();
  });
});
