import { randomUUID } from 'node:crypto';
import { test, expect } from '../../fixtures/api';

test.describe('Indexer category mappings', () => {
  test('upserts and deletes category mappings', async ({ api, publicApi, session }) => {
    if (session.authMode === 'api_key') {
      const unauthorized = await publicApi.POST('/v1/indexers/category-mappings/tracker', {
        body: { tracker_category: 9001, tracker_subcategory: 0, torznab_cat_id: 2000 },
      });
      expect(unauthorized.response.status).toBe(401);
    }

    const trackerUpsert = await api.POST('/v1/indexers/category-mappings/tracker', {
      body: {
        tracker_category: 9001,
        tracker_subcategory: 0,
        torznab_cat_id: 2000,
        media_domain_key: 'movies',
      },
    });
    expect(trackerUpsert.response.status).toBe(204);

    const trackerDelete = await api.DELETE('/v1/indexers/category-mappings/tracker', {
      body: { tracker_category: 9001, tracker_subcategory: 0 },
    });
    expect(trackerDelete.response.status).toBe(204);

    const domainUpsert = await api.POST('/v1/indexers/category-mappings/media-domains', {
      body: { media_domain_key: 'movies', torznab_cat_id: 8000, is_primary: false },
    });
    expect(domainUpsert.response.status).toBe(204);

    const domainDelete = await api.DELETE('/v1/indexers/category-mappings/media-domains', {
      body: { media_domain_key: 'movies', torznab_cat_id: 8000 },
    });
    expect(domainDelete.response.status).toBe(204);

    const definitions = await api.GET('/v1/indexers/definitions');
    expect(definitions.response.ok).toBeTruthy();
    const definition = definitions.data?.definitions?.[0];

    let indexerInstancePublicId = randomUUID();
    let expectedStatus = 404;
    let torznabInstancePublicId = randomUUID();
    let appScopedExpectedStatus = 404;
    if (definition) {
      const createInstance = await api.POST('/v1/indexers/instances', {
        body: {
          indexer_definition_upstream_slug: definition.upstream_slug,
          display_name: `E2E Category Mapping ${Date.now()}`,
        },
      });
      expect(createInstance.response.status).toBe(201);
      indexerInstancePublicId = createInstance.data?.indexer_instance_public_id ?? indexerInstancePublicId;

      const searchProfiles = await api.GET('/v1/indexers/search-profiles');
      expect(searchProfiles.response.ok).toBeTruthy();
      const defaultProfile = searchProfiles.data?.profiles?.find((profile) => profile.is_default);
      expect(defaultProfile).toBeTruthy();

      const torznabCreate = await api.POST('/v1/indexers/torznab-instances', {
        body: {
          display_name: `E2E Category Mapping App ${Date.now()}`,
          search_profile_public_id: defaultProfile?.search_profile_public_id ?? randomUUID(),
        },
      });
      expect([201, 404]).toContain(torznabCreate.response.status);
      torznabInstancePublicId =
        torznabCreate.data?.torznab_instance_public_id ?? torznabInstancePublicId;
      expectedStatus = 204;
      appScopedExpectedStatus = torznabCreate.response.status === 201 ? 204 : 404;
    }

    const instanceTrackerUpsert = await api.POST('/v1/indexers/category-mappings/tracker', {
      body: {
        indexer_instance_public_id: indexerInstancePublicId,
        tracker_category: 9002,
        tracker_subcategory: 1,
        torznab_cat_id: 5000,
        media_domain_key: 'tv',
      },
    });
    expect(instanceTrackerUpsert.response.status).toBe(expectedStatus);

    const instanceTrackerDelete = await api.DELETE('/v1/indexers/category-mappings/tracker', {
      body: {
        indexer_instance_public_id: indexerInstancePublicId,
        tracker_category: 9002,
        tracker_subcategory: 1,
      },
    });
    expect(instanceTrackerDelete.response.status).toBe(expectedStatus);

    const appScopedUpsert = await api.POST('/v1/indexers/category-mappings/tracker', {
      body: {
        torznab_instance_public_id: torznabInstancePublicId,
        indexer_instance_public_id: indexerInstancePublicId,
        tracker_category: 9003,
        tracker_subcategory: 2,
        torznab_cat_id: 2030,
        media_domain_key: 'movies',
      },
    });
    expect(appScopedUpsert.response.status).toBe(appScopedExpectedStatus);

    const appScopedDelete = await api.DELETE('/v1/indexers/category-mappings/tracker', {
      body: {
        torznab_instance_public_id: torznabInstancePublicId,
        indexer_instance_public_id: indexerInstancePublicId,
        tracker_category: 9003,
        tracker_subcategory: 2,
      },
    });
    expect(appScopedDelete.response.status).toBe(appScopedExpectedStatus);
  });
});
