import { test, expect } from '../../fixtures/api';

test.describe('Indexer secrets', () => {
  test('creates, rotates, and revokes secrets', async ({ api, publicApi, session }) => {
    const secretValue = `e2e-secret-${Date.now()}`;

    if (session.authMode === 'api_key') {
      const unauthorized = await publicApi.POST('/v1/indexers/secrets', {
        body: { secret_type: 'api_key', secret_value: secretValue },
      });
      expect(unauthorized.response.status).toBe(401);
    }

    const created = await api.POST('/v1/indexers/secrets', {
      body: { secret_type: 'api_key', secret_value: secretValue },
    });
    expect(created.response.status).toBe(201);
    expect(created.data?.secret_public_id).toBeTruthy();

    const listed = await api.GET('/v1/indexers/secrets');
    expect(listed.response.ok).toBeTruthy();
    expect(
      listed.data?.secrets.some((secret) => secret.secret_public_id === created.data?.secret_public_id),
    ).toBeTruthy();

    const secretPublicId = created.data?.secret_public_id;
    if (!secretPublicId) {
      throw new Error('Missing secret_public_id');
    }

    const rotated = await api.PATCH('/v1/indexers/secrets', {
      body: { secret_public_id: secretPublicId, secret_value: `${secretValue}-rotated` },
    });
    expect(rotated.response.ok).toBeTruthy();
    expect(rotated.data?.secret_public_id).toBe(secretPublicId);

    const revoked = await api.DELETE('/v1/indexers/secrets', {
      body: { secret_public_id: secretPublicId },
    });
    expect(revoked.response.status).toBe(204);
  });
});
