import { randomUUID } from 'node:crypto';
import { test, expect } from '../../fixtures/api';

test.describe('Torznab instances', () => {
  test('covers torznab instance management endpoints', async ({ api, publicApi, session }) => {
    const suffix = Date.now().toString();
    const displayName = `Torznab ${suffix}`;

    const profileCreate = await api.POST('/v1/indexers/search-profiles', {
      body: {
        display_name: `Torznab Profile ${suffix}`,
        page_size: 20,
        default_media_domain_key: 'movies',
      },
    });
    expect(profileCreate.response.status).toBe(201);
    const searchProfileId = profileCreate.data?.search_profile_public_id;
    if (!searchProfileId) {
      throw new Error('Missing search_profile_public_id');
    }

    if (session.authMode === 'api_key') {
      const unauthorized = await publicApi.POST('/v1/indexers/torznab-instances', {
        body: { search_profile_public_id: searchProfileId, display_name: displayName },
      });
      expect(unauthorized.response.status).toBe(401);
    }

    const create = await api.POST('/v1/indexers/torznab-instances', {
      body: { search_profile_public_id: searchProfileId, display_name: displayName },
    });
    const createdInstanceId = create.data?.torznab_instance_public_id;
    const createdApiKey = create.data?.api_key_plaintext;
    const hasCreatedInstance =
      create.response.status === 201 && Boolean(createdInstanceId) && Boolean(createdApiKey);

    const listed = await api.GET('/v1/indexers/torznab-instances');
    expect(listed.response.status).toBe(200);
    if (hasCreatedInstance) {
      expect(
        listed.data?.torznab_instances.some(
          (instance) =>
            instance.torznab_instance_public_id === createdInstanceId &&
            instance.search_profile_public_id === searchProfileId
        )
      ).toBeTruthy();
    }

    if (hasCreatedInstance) {
      const capsMissingKey = await publicApi.GET('/torznab/{torznab_instance_public_id}/api', {
        params: { path: { torznab_instance_public_id: createdInstanceId! }, query: { t: 'caps' } },
      });
      expect(capsMissingKey.response.status).toBe(401);

      const capsInvalidKey = await publicApi.GET('/torznab/{torznab_instance_public_id}/api', {
        params: {
          path: { torznab_instance_public_id: createdInstanceId! },
          query: { apikey: 'invalid', t: 'caps' },
        },
      });
      expect(capsInvalidKey.response.status).toBe(401);

      const caps = await publicApi.GET('/torznab/{torznab_instance_public_id}/api', {
        params: {
          path: { torznab_instance_public_id: createdInstanceId! },
          query: { apikey: createdApiKey!, t: 'caps' },
        },
      });
      expect(caps.response.status).toBe(200);
      const capsBody = await caps.response.text();
      expect(capsBody).toContain('<caps>');

      const unsupportedQuery = await publicApi.GET('/torznab/{torznab_instance_public_id}/api', {
        params: {
          path: { torznab_instance_public_id: createdInstanceId! },
          query: { apikey: createdApiKey!, t: 'invalid-query' },
        },
      });
      expect(unsupportedQuery.response.status).toBe(200);
      const unsupportedBody = await unsupportedQuery.response.text();
      expect(unsupportedBody).toContain('<rss');

      const genericSearch = await publicApi.GET('/torznab/{torznab_instance_public_id}/api', {
        params: {
          path: { torznab_instance_public_id: createdInstanceId! },
          query: { apikey: createdApiKey!, t: 'search', q: 'example', offset: '5', limit: '2' },
        },
      });
      expect(genericSearch.response.status).toBe(200);
      const genericBody = await genericSearch.response.text();
      expect(genericBody).toContain('<rss');
      expect(genericBody).toContain('torznab:response offset="5"');

      const invalidTvCombo = await publicApi.GET('/torznab/{torznab_instance_public_id}/api', {
        params: {
          path: { torznab_instance_public_id: createdInstanceId! },
          query: { apikey: createdApiKey!, t: 'tvsearch', ep: '2' },
        },
      });
      expect(invalidTvCombo.response.status).toBe(200);
      const invalidTvBody = await invalidTvCombo.response.text();
      expect(invalidTvBody).toContain('torznab:response offset="0" total="0"');

      const invalidCategory = await publicApi.GET('/torznab/{torznab_instance_public_id}/api', {
        params: {
          path: { torznab_instance_public_id: createdInstanceId! },
          query: { apikey: createdApiKey!, t: 'search', cat: '999999' },
        },
      });
      expect(invalidCategory.response.status).toBe(200);
      const invalidCategoryBody = await invalidCategory.response.text();
      expect(invalidCategoryBody).toContain('torznab:response offset="0" total="0"');
    }

    const instanceId = randomUUID();
    const sourceId = randomUUID();

    const rotate = await api.PATCH(
      '/v1/indexers/torznab-instances/{torznab_instance_public_id}/rotate',
      {
        params: { path: { torznab_instance_public_id: instanceId } },
      }
    );
    expect(rotate.response.status).toBe(404);

    const updateState = await api.PUT(
      '/v1/indexers/torznab-instances/{torznab_instance_public_id}/state',
      {
        params: { path: { torznab_instance_public_id: instanceId } },
        body: { is_enabled: true },
      }
    );
    expect(updateState.response.status).toBe(404);

    const unknownCaps = await publicApi.GET('/torznab/{torznab_instance_public_id}/api', {
      params: {
        path: { torznab_instance_public_id: instanceId },
        query: { apikey: 'invalid', t: 'caps' },
      },
    });
    expect(unknownCaps.response.status).toBe(404);

    if (session.authMode === 'api_key') {
      const missingKey = await publicApi.GET(
        '/torznab/{torznab_instance_public_id}/download/{canonical_torrent_source_public_id}',
        {
          params: {
            path: {
              torznab_instance_public_id: instanceId,
              canonical_torrent_source_public_id: sourceId,
            },
            query: {},
          },
        }
      );
      expect(missingKey.response.status).toBe(401);
    }

    const download = await publicApi.GET(
      '/torznab/{torznab_instance_public_id}/download/{canonical_torrent_source_public_id}',
      {
        params: {
          path: {
            torznab_instance_public_id: instanceId,
            canonical_torrent_source_public_id: sourceId,
          },
          query: { apikey: 'invalid' },
        },
      }
    );
    expect(download.response.status).toBe(404);

    if (hasCreatedInstance) {
      const disabled = await api.PUT(
        '/v1/indexers/torznab-instances/{torznab_instance_public_id}/state',
        {
          params: { path: { torznab_instance_public_id: createdInstanceId! } },
          body: { is_enabled: false },
        }
      );
      expect(disabled.response.status).toBe(204);

      const listedDisabled = await api.GET('/v1/indexers/torznab-instances');
      expect(listedDisabled.response.status).toBe(200);
      const disabledInstance = listedDisabled.data?.torznab_instances.find(
        (instance) => instance.torznab_instance_public_id === createdInstanceId
      );
      expect(disabledInstance?.is_enabled).toBe(false);

      const createdDownloadMissingKey = await publicApi.GET(
        '/torznab/{torznab_instance_public_id}/download/{canonical_torrent_source_public_id}',
        {
          params: {
            path: {
              torznab_instance_public_id: createdInstanceId!,
              canonical_torrent_source_public_id: sourceId,
            },
            query: {},
          },
        }
      );
      expect(createdDownloadMissingKey.response.status).toBe(401);

      const createdDownloadInvalidKey = await publicApi.GET(
        '/torznab/{torznab_instance_public_id}/download/{canonical_torrent_source_public_id}',
        {
          params: {
            path: {
              torznab_instance_public_id: createdInstanceId!,
              canonical_torrent_source_public_id: sourceId,
            },
            query: { apikey: 'invalid' },
          },
        }
      );
      expect(createdDownloadInvalidKey.response.status).toBe(401);

      const createdDownloadMissingSource = await publicApi.GET(
        '/torznab/{torznab_instance_public_id}/download/{canonical_torrent_source_public_id}',
        {
          params: {
            path: {
              torznab_instance_public_id: createdInstanceId!,
              canonical_torrent_source_public_id: sourceId,
            },
            query: { apikey: createdApiKey! },
          },
        }
      );
      expect(createdDownloadMissingSource.response.status).toBe(404);
    }

    const remove = await api.DELETE(
      '/v1/indexers/torznab-instances/{torznab_instance_public_id}',
      {
        params: { path: { torznab_instance_public_id: instanceId } },
      }
    );
    expect(remove.response.status).toBe(404);
  });
});
