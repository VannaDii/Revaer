import { test, expect } from '../../fixtures/api';

test.describe('Indexer tags', () => {
  test('creates, updates, and deletes tags', async ({ api, publicApi, session }) => {
    const suffix = Date.now().toString();
    const tagKey = `e2e-tag-${suffix}`;
    const displayName = `E2E Tag ${suffix}`;

    if (session.authMode === 'api_key') {
      const unauthorized = await publicApi.POST('/v1/indexers/tags', {
        body: { tag_key: tagKey, display_name: displayName },
      });
      expect(unauthorized.response.status).toBe(401);
    }

    const created = await api.POST('/v1/indexers/tags', {
      body: { tag_key: tagKey, display_name: displayName },
    });
    expect(created.response.status).toBe(201);
    expect(created.data?.tag_public_id).toBeTruthy();
    expect(created.data?.display_name).toBe(displayName);

    const listed = await api.GET('/v1/indexers/tags');
    expect(listed.response.ok).toBeTruthy();
    expect(listed.data?.tags.some((tag) => tag.tag_key === tagKey)).toBeTruthy();

    const tagPublicId = created.data?.tag_public_id;
    if (!tagPublicId) {
      throw new Error('Missing tag_public_id');
    }
    const updatedName = `E2E Tag Updated ${suffix}`;
    const updated = await api.PATCH('/v1/indexers/tags', {
      body: { tag_public_id: tagPublicId, display_name: updatedName },
    });
    expect(updated.response.ok).toBeTruthy();
    expect(updated.data?.display_name).toBe(updatedName);

    const deleted = await api.DELETE(`/v1/indexers/tags/${tagKey}`);
    expect(deleted.response.status).toBe(204);

    const secondKey = `${tagKey}-body`;
    const secondTag = await api.POST('/v1/indexers/tags', {
      body: { tag_key: secondKey, display_name: `${displayName} Body` },
    });
    expect(secondTag.response.status).toBe(201);
    expect(secondTag.data?.tag_public_id).toBeTruthy();

    const deletedByBody = await api.DELETE('/v1/indexers/tags', {
      body: { tag_public_id: secondTag.data?.tag_public_id },
    });
    expect(deletedByBody.response.status).toBe(204);
  });
});
