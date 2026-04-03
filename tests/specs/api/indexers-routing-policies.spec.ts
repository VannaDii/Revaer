import { test, expect } from '../../fixtures/api';

test.describe('Indexer routing policies', () => {
  test('creates routing policies and binds proxy auth secrets', async ({ api, publicApi, session }) => {
    const suffix = Date.now().toString();
    const displayName = `E2E Routing ${suffix}`;

    if (session.authMode === 'api_key') {
      const unauthorized = await publicApi.POST('/v1/indexers/routing-policies', {
        body: { display_name: displayName, mode: 'http_proxy' },
      });
      expect(unauthorized.response.status).toBe(401);
    }

    const created = await api.POST('/v1/indexers/routing-policies', {
      body: { display_name: displayName, mode: 'http_proxy' },
    });
    expect(created.response.status).toBe(201);
    expect(created.data?.routing_policy_public_id).toBeTruthy();
    expect(created.data?.display_name).toBe(displayName);

    const routingPolicyId = created.data?.routing_policy_public_id;
    if (!routingPolicyId) {
      throw new Error('Missing routing_policy_public_id');
    }

    const listed = await api.GET('/v1/indexers/routing-policies');
    expect(listed.response.status).toBe(200);
    expect(
      listed.data?.routing_policies.some(
        (item) =>
          item.routing_policy_public_id === routingPolicyId &&
          item.display_name === displayName
      )
    ).toBe(true);

    const setParam = await api.POST('/v1/indexers/routing-policies/{routing_policy_public_id}/params', {
      params: { path: { routing_policy_public_id: routingPolicyId } },
      body: { param_key: 'proxy_host', value_plain: 'localhost' },
    });
    expect(setParam.response.status).toBe(204);

    const createdSecret = await api.POST('/v1/indexers/secrets', {
      body: { secret_type: 'password', secret_value: `routing-secret-${suffix}` },
    });
    expect(createdSecret.response.status).toBe(201);
    const secretPublicId = createdSecret.data?.secret_public_id;
    if (!secretPublicId) {
      throw new Error('Missing secret_public_id');
    }

    const bindSecret = await api.POST('/v1/indexers/routing-policies/{routing_policy_public_id}/secrets', {
      params: { path: { routing_policy_public_id: routingPolicyId } },
      body: { param_key: 'http_proxy_auth', secret_public_id: secretPublicId },
    });
    expect(bindSecret.response.status).toBe(204);

    const revoked = await api.DELETE('/v1/indexers/secrets', {
      body: { secret_public_id: secretPublicId },
    });
    expect(revoked.response.status).toBe(204);

    const bindRevokedSecret = await api.POST('/v1/indexers/routing-policies/{routing_policy_public_id}/secrets', {
      params: { path: { routing_policy_public_id: routingPolicyId } },
      body: { param_key: 'http_proxy_auth', secret_public_id: secretPublicId },
    });
    expect(bindRevokedSecret.response.status).toBe(404);
  });
});
