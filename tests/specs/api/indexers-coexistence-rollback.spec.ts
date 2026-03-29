import { randomUUID } from 'node:crypto';
import { test, expect } from '../../fixtures/api';

test.describe('Indexer coexistence and rollback', () => {
  test('keeps rollback URL-only and exposes no downstream app mutation surface', async ({
    api,
    publicApi,
  }) => {
    const suffix = randomUUID().slice(0, 8);

    const profileCreate = await api.POST('/v1/indexers/search-profiles', {
      body: {
        display_name: `Rollback Profile ${suffix}`,
        page_size: 20,
        default_media_domain_key: 'movies',
      },
    });
    expect(profileCreate.response.status).toBe(201);
    const profileId = profileCreate.data?.search_profile_public_id;
    if (!profileId) {
      throw new Error('Missing search profile id');
    }

    const profilePatchBeforeImport = await api.PATCH(
      '/v1/indexers/search-profiles/{search_profile_public_id}',
      {
        params: { path: { search_profile_public_id: profileId } },
        body: {
          display_name: `Rollback Profile ${suffix} Before Import`,
          page_size: 25,
        },
      }
    );
    expect(profilePatchBeforeImport.response.ok).toBeTruthy();

    const importJob = await api.POST('/v1/indexers/import-jobs', {
      body: { source: 'prowlarr_backup', is_dry_run: true },
    });
    expect(importJob.response.status).toBe(201);
    const importJobId = importJob.data?.import_job_public_id;
    if (!importJobId) {
      throw new Error('Missing import job id');
    }

    const runImport = await api.POST(
      '/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-backup',
      {
        params: { path: { import_job_public_id: importJobId } },
        body: { backup_blob_ref: `rollback-${suffix}` },
      }
    );
    expect(runImport.response.status).toBe(204);

    const profilePatchAfterImport = await api.PATCH(
      '/v1/indexers/search-profiles/{search_profile_public_id}',
      {
        params: { path: { search_profile_public_id: profileId } },
        body: {
          display_name: `Rollback Profile ${suffix} After Import`,
          page_size: 30,
        },
      }
    );
    expect(profilePatchAfterImport.response.ok).toBeTruthy();

    const secondProfile = await api.POST('/v1/indexers/search-profiles', {
      body: {
        display_name: `Rollback Profile After Import ${suffix}`,
        page_size: 20,
        default_media_domain_key: 'tv',
      },
    });
    expect(secondProfile.response.status).toBe(201);

    const openapi = await publicApi.GET('/docs/openapi.json');
    expect(openapi.response.status).toBe(200);
    const document = openapi.data;
    const pathKeys = Object.keys(
      (document as { paths?: Record<string, unknown> }).paths ?? {}
    ).join('\n');
    expect(pathKeys).not.toContain('/v1/apps');
    expect(pathKeys).not.toContain('radarr');
    expect(pathKeys).not.toContain('sonarr');
    expect(pathKeys).not.toContain('lidarr');
    expect(pathKeys).not.toContain('readarr');
  });
});
