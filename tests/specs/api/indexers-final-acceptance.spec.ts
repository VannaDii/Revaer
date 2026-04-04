import { randomUUID } from 'node:crypto';
import { test, expect } from '../../fixtures/api';
import { buildInsecureTestUrl } from '../../support/urls';

test.describe('Indexer final acceptance', () => {
  test('keeps hard blockers explicit, inspectable, and reversible', async ({
    api,
    publicApi,
    session,
  }) => {
    const suffix = randomUUID().slice(0, 8);

    if (session.authMode === 'api_key') {
      const missingApiKey = await publicApi.POST('/v1/indexers/import-jobs', {
        body: { source: 'prowlarr_backup', is_dry_run: true },
      });
      expect(missingApiKey.response.status).toBe(401);
    }

    const backupJobCreate = await api.POST('/v1/indexers/import-jobs', {
      body: { source: 'prowlarr_backup', is_dry_run: true },
    });
    expect(backupJobCreate.response.status).toBe(201);
    const backupJobId = backupJobCreate.data?.import_job_public_id;
    if (!backupJobId) {
      throw new Error('Missing backup import job id');
    }

    const backupRun = await api.POST(
      '/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-backup',
      {
        params: { path: { import_job_public_id: backupJobId } },
        body: { backup_blob_ref: `acceptance-${suffix}` },
      }
    );
    expect(backupRun.response.status).toBe(204);

    const backupStatus = await api.GET('/v1/indexers/import-jobs/{import_job_public_id}/status', {
      params: { path: { import_job_public_id: backupJobId } },
    });
    expect(backupStatus.response.status).toBe(200);

    const backupResults = await api.GET('/v1/indexers/import-jobs/{import_job_public_id}/results', {
      params: { path: { import_job_public_id: backupJobId } },
    });
    expect(backupResults.response.status).toBe(200);
    expect(Array.isArray(backupResults.data?.results)).toBe(true);

    const apiJobCreate = await api.POST('/v1/indexers/import-jobs', {
      body: { source: 'prowlarr_api', is_dry_run: true },
    });
    expect(apiJobCreate.response.status).toBe(201);
    const apiJobId = apiJobCreate.data?.import_job_public_id;
    if (!apiJobId) {
      throw new Error('Missing api import job id');
    }

    const apiRun = await api.POST('/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-api', {
      params: { path: { import_job_public_id: apiJobId } },
      body: {
        prowlarr_url: buildInsecureTestUrl('prowlarr.local'),
        prowlarr_api_key_secret_public_id: randomUUID(),
      },
    });
    expect(apiRun.response.status).toBe(404);

    const openapi = await publicApi.GET('/docs/openapi.json');
    expect(openapi.response.status).toBe(200);
    const pathMap = (openapi.data as { paths?: Record<string, unknown> }).paths ?? {};
    const pathKeys = Object.keys(pathMap).join('\n');
    expect(pathKeys).toContain('/torznab/{torznab_instance_public_id}/api');
    expect(pathKeys).toContain(
      '/torznab/{torznab_instance_public_id}/download/{canonical_torrent_source_public_id}'
    );
    expect(pathKeys).not.toContain('/v1/apps');
    expect(pathKeys).not.toContain('radarr');
    expect(pathKeys).not.toContain('sonarr');
  });
});
