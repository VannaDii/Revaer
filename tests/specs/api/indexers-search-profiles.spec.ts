import { randomUUID } from 'node:crypto';
import { test, expect } from '../../fixtures/api';

test.describe('Indexer search profiles', () => {
  test('creates and manages search profiles', async ({ api, publicApi, session }) => {
    const suffix = Date.now().toString();
    const displayName = `E2E Search Profile ${suffix}`;

    if (session.authMode === 'api_key') {
      const unauthorized = await publicApi.POST('/v1/indexers/search-profiles', {
        body: { display_name: displayName },
      });
      expect(unauthorized.response.status).toBe(401);
    }

    const created = await api.POST('/v1/indexers/search-profiles', {
      body: { display_name: displayName, page_size: 20, default_media_domain_key: 'movies' },
    });
    expect(created.response.status).toBe(201);
    expect(created.data?.search_profile_public_id).toBeTruthy();

    const profileId = created.data?.search_profile_public_id;
    if (!profileId) {
      throw new Error('Missing search_profile_public_id');
    }

    const updated = await api.PATCH('/v1/indexers/search-profiles/{search_profile_public_id}', {
      params: { path: { search_profile_public_id: profileId } },
      body: { display_name: `${displayName} Updated`, page_size: 25 },
    });
    expect(updated.response.ok).toBeTruthy();

    const listed = await api.GET('/v1/indexers/search-profiles');
    expect(listed.response.status).toBe(200);
    expect(
      listed.data?.search_profiles.some(
        (profile) =>
          profile.search_profile_public_id === profileId &&
          profile.display_name === `${displayName} Updated`
      )
    ).toBeTruthy();

    const setDefault = await api.POST(
      '/v1/indexers/search-profiles/{search_profile_public_id}/default',
      {
        params: { path: { search_profile_public_id: profileId } },
        body: { page_size: 30 },
      }
    );
    expect(setDefault.response.status).toBe(204);

    const setDefaultDomain = await api.PUT(
      '/v1/indexers/search-profiles/{search_profile_public_id}/default-domain',
      {
        params: { path: { search_profile_public_id: profileId } },
        body: { default_media_domain_key: 'tv' },
      }
    );
    expect(setDefaultDomain.response.status).toBe(204);

    const allowlist = await api.PUT(
      '/v1/indexers/search-profiles/{search_profile_public_id}/media-domains',
      {
        params: { path: { search_profile_public_id: profileId } },
        body: { media_domain_keys: ['movies', 'tv'] },
      }
    );
    expect(allowlist.response.status).toBe(204);

    const tagA = await api.POST('/v1/indexers/tags', {
      body: { tag_key: `e2e-tag-a-${suffix}`, display_name: `E2E Tag A ${suffix}` },
    });
    expect(tagA.response.status).toBe(201);
    const tagAPublicId = tagA.data?.tag_public_id;
    if (!tagAPublicId) {
      throw new Error('Missing tag A public id');
    }
    const tagB = await api.POST('/v1/indexers/tags', {
      body: { tag_key: `e2e-tag-b-${suffix}`, display_name: `E2E Tag B ${suffix}` },
    });
    expect(tagB.response.status).toBe(201);
    const tagBPublicId = tagB.data?.tag_public_id;
    if (!tagBPublicId) {
      throw new Error('Missing tag B public id');
    }
    const tagC = await api.POST('/v1/indexers/tags', {
      body: { tag_key: `e2e-tag-c-${suffix}`, display_name: `E2E Tag C ${suffix}` },
    });
    expect(tagC.response.status).toBe(201);
    const tagCPublicId = tagC.data?.tag_public_id;
    if (!tagCPublicId) {
      throw new Error('Missing tag C public id');
    }

    const tagAllow = await api.PUT(
      '/v1/indexers/search-profiles/{search_profile_public_id}/tags/allow',
      {
        params: { path: { search_profile_public_id: profileId } },
        body: { tag_public_ids: [tagAPublicId] },
      }
    );
    expect(tagAllow.response.status).toBe(204);

    const tagBlock = await api.PUT(
      '/v1/indexers/search-profiles/{search_profile_public_id}/tags/block',
      {
        params: { path: { search_profile_public_id: profileId } },
        body: { tag_public_ids: [tagBPublicId] },
      }
    );
    expect(tagBlock.response.status).toBe(204);

    const tagPrefer = await api.PUT(
      '/v1/indexers/search-profiles/{search_profile_public_id}/tags/prefer',
      {
        params: { path: { search_profile_public_id: profileId } },
        body: { tag_public_ids: [tagCPublicId] },
      }
    );
    expect(tagPrefer.response.status).toBe(204);

    const listedAfterTags = await api.GET('/v1/indexers/search-profiles');
    expect(listedAfterTags.response.status).toBe(200);
    const listedProfile = listedAfterTags.data?.search_profiles.find(
      (profile) => profile.search_profile_public_id === profileId
    );
    expect(listedProfile?.allow_tag_keys).toContain(`e2e-tag-a-${suffix}`);
    expect(listedProfile?.block_tag_keys).toContain(`e2e-tag-b-${suffix}`);
    expect(listedProfile?.prefer_tag_keys).toContain(`e2e-tag-c-${suffix}`);

    const policySetAdd = await api.POST(
      '/v1/indexers/search-profiles/{search_profile_public_id}/policy-sets',
      {
        params: { path: { search_profile_public_id: profileId } },
        body: { policy_set_public_id: randomUUID() },
      }
    );
    expect(policySetAdd.response.status).toBe(404);

    const policySetRemove = await api.DELETE(
      '/v1/indexers/search-profiles/{search_profile_public_id}/policy-sets',
      {
        params: { path: { search_profile_public_id: profileId } },
        body: { policy_set_public_id: randomUUID() },
      }
    );
    expect(policySetRemove.response.status).toBe(404);

    const indexerAllow = await api.PUT(
      '/v1/indexers/search-profiles/{search_profile_public_id}/indexers/allow',
      {
        params: { path: { search_profile_public_id: profileId } },
        body: { indexer_instance_public_ids: [randomUUID()] },
      }
    );
    expect(indexerAllow.response.status).toBe(404);

    const indexerBlock = await api.PUT(
      '/v1/indexers/search-profiles/{search_profile_public_id}/indexers/block',
      {
        params: { path: { search_profile_public_id: profileId } },
        body: { indexer_instance_public_ids: [randomUUID()] },
      }
    );
    expect(indexerBlock.response.status).toBe(404);
  });
});
