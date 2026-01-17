import { test, expect } from '../../fixtures/api';

test.describe('Auth', () => {
  test('refresh endpoint matches auth mode', async ({ api, session }) => {
    const response = await api.POST('/v1/auth/refresh');
    if (session.authMode === 'api_key') {
      expect(response.response.ok).toBeTruthy();
      expect(response.data?.api_key_expires_at).toBeTruthy();
    } else {
      expect(response.response.status).toBe(401);
    }
  });

  test('protected endpoints respect API key requirements', async ({ publicApi, session }) => {
    const response = await publicApi.GET('/v1/config');
    if (session.authMode === 'api_key') {
      expect(response.response.status).toBe(401);
    } else {
      expect(response.response.ok).toBeTruthy();
    }
  });
});
