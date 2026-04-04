import { randomUUID } from 'node:crypto';
import { test, expect } from '../../fixtures/api';

test.describe('Indexer instances', () => {
  test('covers instance management endpoints', async ({ api, publicApi, session }) => {
    const displayName = `E2E Instance ${Date.now()}`;
    const definitionUpstreamSlug = 'missing-definition';

    if (session.authMode === 'api_key') {
      const unauthorizedList = await publicApi.GET('/v1/indexers/instances');
      expect(unauthorizedList.response.status).toBe(401);

      const unauthorized = await publicApi.POST('/v1/indexers/instances', {
        body: { indexer_definition_upstream_slug: definitionUpstreamSlug, display_name: displayName },
      });
      expect(unauthorized.response.status).toBe(401);
    }

    const listed = await api.GET('/v1/indexers/instances');
    expect(listed.response.status).toBe(200);
    expect(Array.isArray(listed.data?.indexer_instances)).toBe(true);

    const create = await api.POST('/v1/indexers/instances', {
      body: { indexer_definition_upstream_slug: definitionUpstreamSlug, display_name: displayName },
    });
    expect(create.response.status).toBe(404);

    const instanceId = randomUUID();

    const update = await api.PATCH('/v1/indexers/instances/{indexer_instance_public_id}', {
      params: { path: { indexer_instance_public_id: instanceId } },
      body: { indexer_instance_public_id: instanceId, display_name: `${displayName} Updated` },
    });
    expect(update.response.status).toBe(404);

    const setMediaDomains = await api.PUT(
      '/v1/indexers/instances/{indexer_instance_public_id}/media-domains',
      {
        params: { path: { indexer_instance_public_id: instanceId } },
        body: { media_domain_keys: ['movies'] },
      }
    );
    expect(setMediaDomains.response.status).toBe(404);

    const setTags = await api.PUT('/v1/indexers/instances/{indexer_instance_public_id}/tags', {
      params: { path: { indexer_instance_public_id: instanceId } },
      body: { tag_keys: ['e2e-tag'] },
    });
    expect(setTags.response.status).toBe(404);

    const setFieldValue = await api.PATCH(
      '/v1/indexers/instances/{indexer_instance_public_id}/fields/value',
      {
        params: { path: { indexer_instance_public_id: instanceId } },
        body: { field_name: 'api_key', value_plain: 'e2e-value' },
      }
    );
    expect(setFieldValue.response.status).toBe(404);

    const bindFieldSecret = await api.PATCH(
      '/v1/indexers/instances/{indexer_instance_public_id}/fields/secret',
      {
        params: { path: { indexer_instance_public_id: instanceId } },
        body: { field_name: 'api_key', secret_public_id: randomUUID() },
      }
    );
    expect(bindFieldSecret.response.status).toBe(404);

    const getCfState = await api.GET(
      '/v1/indexers/instances/{indexer_instance_public_id}/cf-state',
      {
        params: { path: { indexer_instance_public_id: instanceId } },
      }
    );
    expect(getCfState.response.status).toBe(404);

    const getConnectivityProfile = await api.GET(
      '/v1/indexers/instances/{indexer_instance_public_id}/connectivity-profile',
      {
        params: { path: { indexer_instance_public_id: instanceId } },
      }
    );
    expect(getConnectivityProfile.response.status).toBe(404);

    const getSourceReputation = await api.GET(
      '/v1/indexers/instances/{indexer_instance_public_id}/reputation',
      {
        params: {
          path: { indexer_instance_public_id: instanceId },
          query: { window_key: '1h', limit: 10 },
        },
      }
    );
    expect(getSourceReputation.response.status).toBe(404);

    const getHealthEvents = await api.GET(
      '/v1/indexers/instances/{indexer_instance_public_id}/health-events',
      {
        params: {
          path: { indexer_instance_public_id: instanceId },
          query: { limit: 10 },
        },
      }
    );
    expect(getHealthEvents.response.status).toBe(404);

    const resetCfState = await api.POST(
      '/v1/indexers/instances/{indexer_instance_public_id}/cf-state/reset',
      {
        params: { path: { indexer_instance_public_id: instanceId } },
        body: { reason: 'e2e reset' },
      }
    );
    expect(resetCfState.response.status).toBe(404);

    const getRssSubscription = await api.GET(
      '/v1/indexers/instances/{indexer_instance_public_id}/rss',
      {
        params: { path: { indexer_instance_public_id: instanceId } },
      }
    );
    expect(getRssSubscription.response.status).toBe(404);

    const updateRssSubscription = await api.PUT(
      '/v1/indexers/instances/{indexer_instance_public_id}/rss',
      {
        params: { path: { indexer_instance_public_id: instanceId } },
        body: { is_enabled: true, interval_seconds: 900 },
      }
    );
    expect(updateRssSubscription.response.status).toBe(404);

    const getRssItems = await api.GET(
      '/v1/indexers/instances/{indexer_instance_public_id}/rss/items',
      {
        params: { path: { indexer_instance_public_id: instanceId }, query: { limit: 10 } },
      }
    );
    expect(getRssItems.response.status).toBe(404);

    const markRssItemSeen = await api.POST(
      '/v1/indexers/instances/{indexer_instance_public_id}/rss/items',
      {
        params: { path: { indexer_instance_public_id: instanceId } },
        body: { item_guid: 'e2e-guid' },
      }
    );
    expect(markRssItemSeen.response.status).toBe(404);
  });
});
