import { test, expect } from '../../fixtures/api';
import { randomUUID } from 'crypto';
import fs from 'fs';
import path from 'path';
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

let torrentId: string;
let adminTorrentId: string;
let authorRoot = '';

test.describe.serial('Torrent endpoints', () => {
  test.beforeAll(() => {
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

  test('creates torrents via v1 and admin', async ({ api }) => {
    torrentId = randomUUID();
    adminTorrentId = randomUUID();

    const create = await api.POST('/v1/torrents', {
      body: {
        id: torrentId,
        magnet: MAGNET_URI,
        trackers: [TRACKER_PRIMARY],
      },
    });
    expect(create.response.status).toBe(202);

    const createAdmin = await api.POST('/admin/torrents', {
      body: {
        id: adminTorrentId,
        magnet: MAGNET_URI,
        trackers: [TRACKER_PRIMARY],
      },
    });
    expect(createAdmin.response.status).toBe(202);
  });

  test('lists torrents via v1 and admin', async ({ api }) => {
    await expect.poll(async () => {
      const list = await api.GET('/v1/torrents');
      if (!list.response.ok) {
        return false;
      }
      return list.data?.torrents?.some((entry) => entry.id === torrentId) ?? false;
    }).toBeTruthy();

    await expect.poll(async () => {
      const adminList = await api.GET('/admin/torrents');
      if (!adminList.response.ok) {
        return false;
      }
      return adminList.data?.torrents?.some((entry) => entry.id === torrentId) ?? false;
    }).toBeTruthy();
  });

  test('gets torrent details via v1 and admin', async ({ api }) => {
    const detail = await api.GET('/v1/torrents/{id}', {
      params: { path: { id: torrentId } },
    });
    expect(detail.response.ok).toBeTruthy();
    expect(detail.data?.id).toBe(torrentId);

    const adminDetail = await api.GET('/admin/torrents/{id}', {
      params: { path: { id: torrentId } },
    });
    expect(adminDetail.response.ok).toBeTruthy();
    expect(adminDetail.data?.id).toBe(torrentId);
  });

  test('updates categories and tags via v1 and admin', async ({ api }) => {
    const category = await api.PUT('/v1/torrents/categories/{name}', {
      params: { path: { name: CATEGORY_V1 } },
      body: {},
    });
    expect(category.response.ok).toBeTruthy();

    const categories = await api.GET('/v1/torrents/categories');
    expect(categories.response.ok).toBeTruthy();
    expect(categories.data?.some((entry) => entry.name === CATEGORY_V1)).toBeTruthy();

    const adminCategory = await api.PUT('/admin/torrents/categories/{name}', {
      params: { path: { name: CATEGORY_ADMIN } },
      body: {},
    });
    expect(adminCategory.response.ok).toBeTruthy();

    const adminCategories = await api.GET('/admin/torrents/categories');
    expect(adminCategories.response.ok).toBeTruthy();
    expect(
      adminCategories.data?.some((entry) => entry.name === CATEGORY_ADMIN),
    ).toBeTruthy();

    const tag = await api.PUT('/v1/torrents/tags/{name}', {
      params: { path: { name: TAG_V1 } },
      body: {},
    });
    expect(tag.response.ok).toBeTruthy();

    const tags = await api.GET('/v1/torrents/tags');
    expect(tags.response.ok).toBeTruthy();
    expect(tags.data?.some((entry) => entry.name === TAG_V1)).toBeTruthy();

    const adminTag = await api.PUT('/admin/torrents/tags/{name}', {
      params: { path: { name: TAG_ADMIN } },
      body: {},
    });
    expect(adminTag.response.ok).toBeTruthy();

    const adminTags = await api.GET('/admin/torrents/tags');
    expect(adminTags.response.ok).toBeTruthy();
    expect(adminTags.data?.some((entry) => entry.name === TAG_ADMIN)).toBeTruthy();
  });

  test('authors torrents via v1 and admin', async ({ api }) => {
    const author = await api.POST('/v1/torrents/create', {
      body: { root_path: authorRoot },
    });
    if (!author.response.ok) {
      throw new Error(`Authoring failed: ${author.response.status}`);
    }
    expect(author.data?.metainfo).toBeTruthy();
    expect(author.data?.magnet_uri).toBeTruthy();

    const adminAuthor = await api.POST('/admin/torrents/create', {
      body: { root_path: authorRoot },
    });
    if (!adminAuthor.response.ok) {
      throw new Error(`Admin authoring failed: ${adminAuthor.response.status}`);
    }
  });

  test('rejects authoring without root path', async ({ api }) => {
    const response = await api.POST('/v1/torrents/create', {
      body: { root_path: '' },
    });
    expect(response.response.status).toBe(400);
  });

  test('updates selection, options, and actions', async ({ api }) => {
    const selection = await api.POST('/v1/torrents/{id}/select', {
      params: { path: { id: torrentId } },
      body: {
        include: ['*.mkv'],
        exclude: [],
        skip_fluff: false,
        priorities: [],
      },
    });
    expect(selection.response.status).toBe(202);

    const options = await api.PATCH('/v1/torrents/{id}/options', {
      params: { path: { id: torrentId } },
      body: { auto_managed: true },
    });
    expect(options.response.status).toBe(202);

    const action = await api.POST('/v1/torrents/{id}/action', {
      params: { path: { id: torrentId } },
      body: { type: 'pause' },
    });
    expect(action.response.status).toBe(202);
  });

  test('rejects empty options payload', async ({ api }) => {
    const response = await api.PATCH('/v1/torrents/{id}/options', {
      params: { path: { id: torrentId } },
      body: {},
    });
    expect(response.response.status).toBe(400);
  });

  test('updates trackers and web seeds', async ({ api }) => {
    const updateTrackers = await api.PATCH('/v1/torrents/{id}/trackers', {
      params: { path: { id: torrentId } },
      body: { trackers: [TRACKER_SECONDARY], replace: false },
    });
    expect(updateTrackers.response.status).toBe(202);

    const listTrackers = await api.GET('/v1/torrents/{id}/trackers', {
      params: { path: { id: torrentId } },
    });
    expect(listTrackers.response.ok).toBeTruthy();

    const removeTrackers = await api.DELETE('/v1/torrents/{id}/trackers', {
      params: { path: { id: torrentId } },
      body: { trackers: [TRACKER_PRIMARY] },
    });
    expect(removeTrackers.response.status).toBe(202);

    const updateSeeds = await api.PATCH('/v1/torrents/{id}/web_seeds', {
      params: { path: { id: torrentId } },
      body: { web_seeds: [WEB_SEED], replace: true },
    });
    expect(updateSeeds.response.status).toBe(202);
  });

  test('rejects empty tracker and web seed updates', async ({ api }) => {
    const updateTrackers = await api.PATCH('/v1/torrents/{id}/trackers', {
      params: { path: { id: torrentId } },
      body: { trackers: [], replace: false },
    });
    expect(updateTrackers.response.status).toBe(400);

    const removeTrackers = await api.DELETE('/v1/torrents/{id}/trackers', {
      params: { path: { id: torrentId } },
      body: { trackers: [] },
    });
    expect(removeTrackers.response.status).toBe(400);

    const updateSeeds = await api.PATCH('/v1/torrents/{id}/web_seeds', {
      params: { path: { id: torrentId } },
      body: { web_seeds: [], replace: false },
    });
    expect(updateSeeds.response.status).toBe(400);
  });

  test('lists peers via v1 and admin', async ({ api }) => {
    const peers = await api.GET('/v1/torrents/{id}/peers', {
      params: { path: { id: torrentId } },
    });
    expect(peers.response.ok).toBeTruthy();

    const adminPeers = await api.GET('/admin/torrents/{id}/peers', {
      params: { path: { id: torrentId } },
    });
    expect(adminPeers.response.ok).toBeTruthy();
  });

  test('deletes admin torrent', async ({ api }) => {
    const response = await api.DELETE('/admin/torrents/{id}', {
      params: { path: { id: adminTorrentId } },
    });
    expect(response.response.status).toBe(204);
  });
});
