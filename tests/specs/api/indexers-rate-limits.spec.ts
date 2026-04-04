import { randomUUID } from 'node:crypto';
import { test, expect } from '../../fixtures/api';

test.describe('Indexer rate limits', () => {
  test('creates, updates, and assigns rate limit policies', async ({ api, publicApi, session }) => {
    const suffix = Date.now().toString();
    const displayName = `E2E Rate Limit ${suffix}`;

    if (session.authMode === 'api_key') {
      const unauthorized = await publicApi.POST('/v1/indexers/rate-limits', {
        body: { display_name: displayName, rpm: 120, burst: 30, concurrent: 2 },
      });
      expect(unauthorized.response.status).toBe(401);
    }

    const created = await api.POST('/v1/indexers/rate-limits', {
      body: { display_name: displayName, rpm: 120, burst: 30, concurrent: 2 },
    });
    expect(created.response.status).toBe(201);
    expect(created.data?.rate_limit_policy_public_id).toBeTruthy();

    const policyId = created.data?.rate_limit_policy_public_id;
    if (!policyId) {
      throw new Error('Missing rate_limit_policy_public_id');
    }

    const listed = await api.GET('/v1/indexers/rate-limits');
    expect(listed.response.status).toBe(200);
    expect(
      listed.data?.rate_limit_policies.some(
        (item) =>
          item.rate_limit_policy_public_id === policyId &&
          item.display_name === displayName
      )
    ).toBe(true);

    const update = await api.PATCH('/v1/indexers/rate-limits/{rate_limit_policy_public_id}', {
      params: { path: { rate_limit_policy_public_id: policyId } },
      body: { display_name: `${displayName} Updated`, rpm: 240 },
    });
    expect(update.response.status).toBe(204);

    const setInstance = await api.PUT(
      '/v1/indexers/instances/{indexer_instance_public_id}/rate-limit',
      {
        params: { path: { indexer_instance_public_id: randomUUID() } },
        body: { rate_limit_policy_public_id: policyId },
      }
    );
    expect(setInstance.response.status).toBe(404);

    const setRouting = await api.PUT(
      '/v1/indexers/routing-policies/{routing_policy_public_id}/rate-limit',
      {
        params: { path: { routing_policy_public_id: randomUUID() } },
        body: { rate_limit_policy_public_id: policyId },
      }
    );
    expect(setRouting.response.status).toBe(404);

    const deleted = await api.DELETE(
      '/v1/indexers/rate-limits/{rate_limit_policy_public_id}',
      {
        params: { path: { rate_limit_policy_public_id: policyId } },
      }
    );
    expect(deleted.response.status).toBe(204);
  });
});
