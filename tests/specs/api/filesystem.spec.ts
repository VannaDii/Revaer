import { test, expect } from '../../fixtures/api';
import { resolveFsRoot } from '../../support/paths';

test.describe('Filesystem browse', () => {
  test('lists entries under configured root', async ({ api }) => {
    const rootPath = resolveFsRoot();
    const response = await api.GET('/v1/fs/browse', {
      params: { query: { path: rootPath } },
    });
    expect(response.response.ok).toBeTruthy();
    expect(response.data?.path).toBeTruthy();
    expect(Array.isArray(response.data?.entries)).toBeTruthy();
  });
});
