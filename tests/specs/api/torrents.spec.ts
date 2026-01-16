import { test, expect } from '@playwright/test';
import { randomUUID } from 'crypto';
import fs from 'fs';
import path from 'path';
import { authHeaders } from '../../support/headers';
import { loadSession, type ApiSession } from '../../support/session';
import { resolveFsRoot } from '../../support/paths';

const MAGNET_URI =
  'magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567';
const TRACKER_PRIMARY = 'https://tracker.example/announce';
const TRACKER_SECONDARY = 'https://tracker.example/alt';
const WEB_SEED = 'https://seed.example/file';
const CATEGORY_V1 = 'e2e-category';
const CATEGORY_ADMIN = 'e2e-admin-category';
const TAG_V1 = 'e2e-tag';
const TAG_ADMIN = 'e2e-admin-tag';

let session: ApiSession;
let torrentId: string;
let adminTorrentId: string;
let authorRoot = '';

test.describe.serial('Torrent endpoints', () => {
  test.beforeAll(() => {
    session = loadSession();
    const resolvedRoot = resolveFsRoot();
    fs.mkdirSync(resolvedRoot, { recursive: true });
    const prefix = path.join(resolvedRoot, 'e2e-author-');
    authorRoot = fs.mkdtempSync(prefix);
    fs.writeFileSync(path.join(authorRoot, 'seed.txt'), 'revaer e2e');
  });

  test.afterAll(() => {
    if (authorRoot) {
      fs.rmSync(authorRoot, { recursive: true, force: true });
    }
  });

  test('creates torrents via v1 and admin', async ({ request }) => {
    torrentId = randomUUID();
    adminTorrentId = randomUUID();

    const create = await request.post('/v1/torrents', {
      data: {
        id: torrentId,
        magnet: MAGNET_URI,
        trackers: [TRACKER_PRIMARY],
      },
      headers: authHeaders(session),
    });
    expect(create.status()).toBe(202);

    const createAdmin = await request.post('/admin/torrents', {
      data: {
        id: adminTorrentId,
        magnet: MAGNET_URI,
        trackers: [TRACKER_PRIMARY],
      },
      headers: authHeaders(session),
    });
    expect(createAdmin.status()).toBe(202);
  });

  test('lists torrents via v1 and admin', async ({ request }) => {
    await expect.poll(async () => {
      const list = await request.get('/v1/torrents', {
        headers: authHeaders(session),
      });
      if (!list.ok()) {
        return false;
      }
      const listBody = (await list.json()) as { torrents?: Array<{ id?: string }> };
      return listBody.torrents?.some((entry) => entry.id === torrentId) ?? false;
    }).toBeTruthy();

    await expect.poll(async () => {
      const adminList = await request.get('/admin/torrents', {
        headers: authHeaders(session),
      });
      if (!adminList.ok()) {
        return false;
      }
      const adminBody = (await adminList.json()) as { torrents?: Array<{ id?: string }> };
      return adminBody.torrents?.some((entry) => entry.id === torrentId) ?? false;
    }).toBeTruthy();
  });

  test('gets torrent details via v1 and admin', async ({ request }) => {
    const detail = await request.get(`/v1/torrents/${torrentId}`, {
      headers: authHeaders(session),
    });
    expect(detail.ok()).toBeTruthy();
    const detailBody = (await detail.json()) as { id?: string };
    expect(detailBody.id).toBe(torrentId);

    const adminDetail = await request.get(`/admin/torrents/${torrentId}`, {
      headers: authHeaders(session),
    });
    expect(adminDetail.ok()).toBeTruthy();
    const adminDetailBody = (await adminDetail.json()) as { id?: string };
    expect(adminDetailBody.id).toBe(torrentId);
  });

  test('updates categories and tags via v1 and admin', async ({ request }) => {
    const category = await request.put(`/v1/torrents/categories/${CATEGORY_V1}`, {
      data: {},
      headers: authHeaders(session),
    });
    expect(category.ok()).toBeTruthy();

    const categories = await request.get('/v1/torrents/categories', {
      headers: authHeaders(session),
    });
    expect(categories.ok()).toBeTruthy();
    const categoriesBody = (await categories.json()) as Array<{ name?: string }>;
    expect(categoriesBody.some((entry) => entry.name === CATEGORY_V1)).toBeTruthy();

    const adminCategory = await request.put(
      `/admin/torrents/categories/${CATEGORY_ADMIN}`,
      {
        data: {},
        headers: authHeaders(session),
      },
    );
    expect(adminCategory.ok()).toBeTruthy();

    const adminCategories = await request.get('/admin/torrents/categories', {
      headers: authHeaders(session),
    });
    expect(adminCategories.ok()).toBeTruthy();
    const adminCategoriesBody = (await adminCategories.json()) as Array<{ name?: string }>;
    expect(
      adminCategoriesBody.some((entry) => entry.name === CATEGORY_ADMIN),
    ).toBeTruthy();

    const tag = await request.put(`/v1/torrents/tags/${TAG_V1}`, {
      data: {},
      headers: authHeaders(session),
    });
    expect(tag.ok()).toBeTruthy();

    const tags = await request.get('/v1/torrents/tags', {
      headers: authHeaders(session),
    });
    expect(tags.ok()).toBeTruthy();
    const tagsBody = (await tags.json()) as Array<{ name?: string }>;
    expect(tagsBody.some((entry) => entry.name === TAG_V1)).toBeTruthy();

    const adminTag = await request.put(`/admin/torrents/tags/${TAG_ADMIN}`, {
      data: {},
      headers: authHeaders(session),
    });
    expect(adminTag.ok()).toBeTruthy();

    const adminTags = await request.get('/admin/torrents/tags', {
      headers: authHeaders(session),
    });
    expect(adminTags.ok()).toBeTruthy();
    const adminTagsBody = (await adminTags.json()) as Array<{ name?: string }>;
    expect(adminTagsBody.some((entry) => entry.name === TAG_ADMIN)).toBeTruthy();
  });

  test('authors torrents via v1 and admin', async ({ request }) => {
    const author = await request.post('/v1/torrents/create', {
      data: { root_path: authorRoot },
      headers: authHeaders(session),
    });
    if (!author.ok()) {
      const body = await author.text();
      throw new Error(`Authoring failed: ${author.status()} ${body}`);
    }
    const authorBody = (await author.json()) as { metainfo?: string; magnet_uri?: string };
    expect(authorBody.metainfo).toBeTruthy();
    expect(authorBody.magnet_uri).toBeTruthy();

    const adminAuthor = await request.post('/admin/torrents/create', {
      data: { root_path: authorRoot },
      headers: authHeaders(session),
    });
    if (!adminAuthor.ok()) {
      const body = await adminAuthor.text();
      throw new Error(`Admin authoring failed: ${adminAuthor.status()} ${body}`);
    }
  });

  test('rejects authoring without root path', async ({ request }) => {
    const response = await request.post('/v1/torrents/create', {
      data: { root_path: '' },
      headers: authHeaders(session),
    });
    expect(response.status()).toBe(400);
  });

  test('updates selection, options, and actions', async ({ request }) => {
    const selection = await request.post(`/v1/torrents/${torrentId}/select`, {
      data: { include: ['*.mkv'], exclude: [], skip_fluff: false, priorities: [] },
      headers: authHeaders(session),
    });
    expect(selection.status()).toBe(202);

    const options = await request.patch(`/v1/torrents/${torrentId}/options`, {
      data: { auto_managed: true },
      headers: authHeaders(session),
    });
    expect(options.status()).toBe(202);

    const action = await request.post(`/v1/torrents/${torrentId}/action`, {
      data: { type: 'pause' },
      headers: authHeaders(session),
    });
    expect(action.status()).toBe(202);
  });

  test('rejects empty options payload', async ({ request }) => {
    const response = await request.patch(`/v1/torrents/${torrentId}/options`, {
      data: {},
      headers: authHeaders(session),
    });
    expect(response.status()).toBe(400);
  });

  test('updates trackers and web seeds', async ({ request }) => {
    const updateTrackers = await request.patch(
      `/v1/torrents/${torrentId}/trackers`,
      {
        data: { trackers: [TRACKER_SECONDARY], replace: false },
        headers: authHeaders(session),
      },
    );
    expect(updateTrackers.status()).toBe(202);

    const listTrackers = await request.get(`/v1/torrents/${torrentId}/trackers`, {
      headers: authHeaders(session),
    });
    expect(listTrackers.ok()).toBeTruthy();

    const removeTrackers = await request.delete(
      `/v1/torrents/${torrentId}/trackers`,
      {
        data: { trackers: [TRACKER_PRIMARY] },
        headers: authHeaders(session),
      },
    );
    expect(removeTrackers.status()).toBe(202);

    const updateSeeds = await request.patch(
      `/v1/torrents/${torrentId}/web_seeds`,
      {
        data: { web_seeds: [WEB_SEED], replace: true },
        headers: authHeaders(session),
      },
    );
    expect(updateSeeds.status()).toBe(202);
  });

  test('rejects empty tracker and web seed updates', async ({ request }) => {
    const updateTrackers = await request.patch(
      `/v1/torrents/${torrentId}/trackers`,
      {
        data: { trackers: [], replace: false },
        headers: authHeaders(session),
      },
    );
    expect(updateTrackers.status()).toBe(400);

    const removeTrackers = await request.delete(
      `/v1/torrents/${torrentId}/trackers`,
      {
        data: { trackers: [] },
        headers: authHeaders(session),
      },
    );
    expect(removeTrackers.status()).toBe(400);

    const updateSeeds = await request.patch(
      `/v1/torrents/${torrentId}/web_seeds`,
      {
        data: { web_seeds: [], replace: false },
        headers: authHeaders(session),
      },
    );
    expect(updateSeeds.status()).toBe(400);
  });

  test('lists peers via v1 and admin', async ({ request }) => {
    const peers = await request.get(`/v1/torrents/${torrentId}/peers`, {
      headers: authHeaders(session),
    });
    expect(peers.ok()).toBeTruthy();

    const adminPeers = await request.get(`/admin/torrents/${torrentId}/peers`, {
      headers: authHeaders(session),
    });
    expect(adminPeers.ok()).toBeTruthy();
  });

  test('deletes admin torrent', async ({ request }) => {
    const response = await request.delete(`/admin/torrents/${adminTorrentId}`, {
      headers: authHeaders(session),
    });
    expect(response.status()).toBe(204);
  });
});
