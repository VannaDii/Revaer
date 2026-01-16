import { test, expect } from '@playwright/test';
import { authHeaders } from '../../support/headers';
import { loadSession, type ApiSession } from '../../support/session';
import { resolveFsRoot } from '../../support/paths';

let session: ApiSession;

test.beforeAll(() => {
  session = loadSession();
});

test.describe('Filesystem browse', () => {
  test('lists entries under configured root', async ({ request }) => {
    const rootPath = resolveFsRoot();
    const response = await request.get(
      `/v1/fs/browse?path=${encodeURIComponent(rootPath)}`,
      {
        headers: authHeaders(session),
      },
    );
    expect(response.ok()).toBeTruthy();

    const body = (await response.json()) as {
      path?: string;
      entries?: unknown[];
    };
    expect(body.path).toBeTruthy();
    expect(Array.isArray(body.entries)).toBeTruthy();
  });
});
