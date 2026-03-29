import { randomUUID } from 'crypto';
import { test, expect } from '../../fixtures/api';

test.describe('Indexer import jobs', () => {
  test('covers import job endpoints', async ({ api, publicApi, session }) => {
    if (session.authMode === 'api_key') {
      const unauthorized = await publicApi.POST('/v1/indexers/import-jobs', {
        body: { source: 'prowlarr_api' },
      });
      expect(unauthorized.response.status).toBe(401);
    }

    const createApiJob = await api.POST('/v1/indexers/import-jobs', {
      body: { source: 'prowlarr_api', is_dry_run: true },
    });
    expect(createApiJob.response.status).toBe(201);

    const apiImportJobId = createApiJob.data?.import_job_public_id ?? randomUUID();

    const runProwlarrApi = await api.POST(
      '/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-api',
      {
        params: { path: { import_job_public_id: apiImportJobId } },
        body: {
          prowlarr_url: 'http://prowlarr.local',
          prowlarr_api_key_secret_public_id: randomUUID(),
        },
      }
    );
    expect(runProwlarrApi.response.status).toBe(404);

    const runProwlarrBackup = await api.POST(
      '/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-backup',
      {
        params: { path: { import_job_public_id: apiImportJobId } },
        body: { backup_blob_ref: 'e2e-backup' },
      }
    );
    expect(runProwlarrBackup.response.status).toBe(409);

    const createBackupJob = await api.POST('/v1/indexers/import-jobs', {
      body: { source: 'prowlarr_backup', is_dry_run: true },
    });
    expect(createBackupJob.response.status).toBe(201);
    const backupImportJobId = createBackupJob.data?.import_job_public_id ?? randomUUID();

    const runBackupSource = await api.POST(
      '/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-backup',
      {
        params: { path: { import_job_public_id: backupImportJobId } },
        body: { backup_blob_ref: 'e2e-backup' },
      }
    );
    expect(runBackupSource.response.status).toBe(204);

    const runApiOnBackupSource = await api.POST(
      '/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-api',
      {
        params: { path: { import_job_public_id: backupImportJobId } },
        body: {
          prowlarr_url: 'http://prowlarr.local',
          prowlarr_api_key_secret_public_id: randomUUID(),
        },
      }
    );
    expect(runApiOnBackupSource.response.status).toBe(409);

    const status = await api.GET('/v1/indexers/import-jobs/{import_job_public_id}/status', {
      params: { path: { import_job_public_id: apiImportJobId } },
    });
    expect(status.response.status).toBe(200);

    const results = await api.GET('/v1/indexers/import-jobs/{import_job_public_id}/results', {
      params: { path: { import_job_public_id: apiImportJobId } },
    });
    expect(results.response.status).toBe(200);
    expect(Array.isArray(results.data?.results)).toBe(true);
  });
});
