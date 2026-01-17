import { test, expect } from '../../fixtures/api';

test.describe('Setup lifecycle', () => {
  test('setup endpoints reject after activation', async ({ publicApi }) => {
    const start = await publicApi.POST('/admin/setup/start', { body: {} });
    expect(start.response.status).toBe(409);

    const complete = await publicApi.POST('/admin/setup/complete', { body: {} });
    expect(complete.response.status).toBe(409);
  });
});
