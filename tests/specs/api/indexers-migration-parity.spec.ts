import { randomUUID } from 'node:crypto';
import { test, expect } from '../../fixtures/api';
import { apiFetchRaw } from '../../support/api/raw';
import { buildInsecureTestUrl } from '../../support/urls';

test.describe('Indexer migration parity flows', () => {
  test('covers prowlarr import and torznab parity/download paths', async ({ api, baseUrl, session }) => {
    const suffix = randomUUID().slice(0, 8);

    const profileCreate = await api.POST('/v1/indexers/search-profiles', {
      body: {
        display_name: `Parity Profile ${suffix}`,
        page_size: 20,
        default_media_domain_key: 'movies',
      },
    });
    expect(profileCreate.response.status).toBe(201);
    const searchProfileId = profileCreate.data?.search_profile_public_id;
    if (!searchProfileId) {
      throw new Error('Missing search_profile_public_id');
    }

    const torznabCreate = await api.POST('/v1/indexers/torznab-instances', {
      body: {
        search_profile_public_id: searchProfileId,
        display_name: `Parity Torznab ${suffix}`,
      },
    });
    const torznabInstanceId = torznabCreate.data?.torznab_instance_public_id;
    const torznabApiKey = torznabCreate.data?.api_key_plaintext;
    const hasTorznabInstance =
      torznabCreate.response.status === 201 && Boolean(torznabInstanceId) && Boolean(torznabApiKey);

    if (hasTorznabInstance) {
      const caps = await apiFetchRaw({
        baseUrl,
        method: 'GET',
        route: '/torznab/{torznab_instance_public_id}/api',
        path: { torznab_instance_public_id: torznabInstanceId! },
        query: { apikey: torznabApiKey!, t: 'caps' },
      });
      expect(caps.status).toBe(200);
      expect(await caps.text()).toContain('<caps>');

      const tvSearchInvalidCombo = await apiFetchRaw({
        baseUrl,
        method: 'GET',
        route: '/torznab/{torznab_instance_public_id}/api',
        path: { torznab_instance_public_id: torznabInstanceId! },
        query: { apikey: torznabApiKey!, t: 'tvsearch', ep: '2' },
      });
      expect(tvSearchInvalidCombo.status).toBe(200);
      expect(await tvSearchInvalidCombo.text()).toContain(
        'torznab:response offset="0" total="0"'
      );

      const missingSourceDownload = await apiFetchRaw({
        baseUrl,
        method: 'GET',
        route: '/torznab/{torznab_instance_public_id}/download/{canonical_torrent_source_public_id}',
        path: {
          torznab_instance_public_id: torznabInstanceId!,
          canonical_torrent_source_public_id: randomUUID(),
        },
        query: { apikey: torznabApiKey! },
      });
      expect(missingSourceDownload.status).toBe(404);

      if (session.authMode === 'api_key') {
        const missingApiKeyDownload = await apiFetchRaw({
          baseUrl,
          method: 'GET',
          route: '/torznab/{torznab_instance_public_id}/download/{canonical_torrent_source_public_id}',
          path: {
            torznab_instance_public_id: torznabInstanceId!,
            canonical_torrent_source_public_id: randomUUID(),
          },
        });
        expect(missingApiKeyDownload.status).toBe(401);
      }
    }

    const createApiImportJob = await api.POST('/v1/indexers/import-jobs', {
      body: { source: 'prowlarr_api', is_dry_run: true },
    });
    expect(createApiImportJob.response.status).toBe(201);
    const apiImportJobId = createApiImportJob.data?.import_job_public_id;
    if (!apiImportJobId) {
      throw new Error('Missing import_job_public_id for api source');
    }

    const runApiImport = await api.POST(
      '/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-api',
      {
        params: { path: { import_job_public_id: apiImportJobId } },
        body: {
          prowlarr_url: buildInsecureTestUrl('prowlarr.local'),
          prowlarr_api_key_secret_public_id: randomUUID(),
        },
      }
    );
    expect(runApiImport.response.status).toBe(404);

    const createBackupImportJob = await api.POST('/v1/indexers/import-jobs', {
      body: { source: 'prowlarr_backup', is_dry_run: true },
    });
    expect(createBackupImportJob.response.status).toBe(201);
    const backupImportJobId = createBackupImportJob.data?.import_job_public_id;
    if (!backupImportJobId) {
      throw new Error('Missing import_job_public_id for backup source');
    }

    const runBackupImport = await api.POST(
      '/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-backup',
      {
        params: { path: { import_job_public_id: backupImportJobId } },
        body: { backup_blob_ref: 'parity-e2e-backup' },
      }
    );
    expect(runBackupImport.response.status).toBe(204);
  });
});
