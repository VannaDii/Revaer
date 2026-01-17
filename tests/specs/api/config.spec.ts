import { test, expect } from '../../fixtures/api';

test.describe('Config and dashboard', () => {
  test('fetches dashboard snapshot', async ({ api }) => {
    const response = await api.GET('/v1/dashboard');
    expect(response.response.ok).toBeTruthy();
    expect(response.data?.download_bps).toBeDefined();
    expect(response.data?.upload_bps).toBeDefined();
  });

  test('gets and patches config snapshot', async ({ api }) => {
    const snapshot = await api.GET('/v1/config');
    expect(snapshot.response.ok).toBeTruthy();

    const patch = await api.PATCH('/v1/config', { body: {} });
    expect(patch.response.ok).toBeTruthy();
    expect(patch.data?.revision).toBeDefined();
  });

  test('applies admin settings patch', async ({ api }) => {
    const response = await api.PATCH('/admin/settings', { body: {} });
    expect(response.response.ok).toBeTruthy();
    expect(response.data?.revision).toBeDefined();
  });
});
