import { test, expect } from '@playwright/test';


test.describe('Setup lifecycle', () => {
  test('setup endpoints reject after activation', async ({ request }) => {
    const start = await request.post('/admin/setup/start', { data: {} });
    expect(start.status()).toBe(409);

    const complete = await request.post('/admin/setup/complete', { data: {} });
    expect(complete.status()).toBe(409);
  });
});
