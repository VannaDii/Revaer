import { test, expect } from '../../fixtures/api';

test.describe('Indexer definitions', () => {
  test('lists indexer definitions', async ({ api, publicApi, session }) => {
    if (session.authMode === 'api_key') {
      const unauthorized = await publicApi.GET('/v1/indexers/definitions');
      expect(unauthorized.response.status).toBe(401);
    }

    const listed = await api.GET('/v1/indexers/definitions');
    expect(listed.response.ok).toBeTruthy();
    const definitions = listed.data?.definitions ?? [];
    for (const definition of definitions) {
      expect(definition.upstream_source).toBeTruthy();
      expect(definition.upstream_slug).toBeTruthy();
      expect(definition.display_name).toBeTruthy();
      expect(definition.definition_hash).toHaveLength(64);
    }
  });
});
