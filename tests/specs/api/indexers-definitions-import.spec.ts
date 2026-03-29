import { test, expect } from '../../fixtures/api';

test.describe('Indexer definition imports', () => {
  test('imports Cardigann YAML into the catalog', async ({ api, publicApi, session }) => {
    const slug = `cardigann-e2e-${Date.now()}`;
    const yamlPayload = `
id: ${slug}
name: Cardigann E2E ${slug}
caps:
  search:
    - q
settings:
  - name: apiKey
    label: API key
    type: apikey
    required: true
  - name: sort
    type: select
    default: seeders
    options:
      - value: seeders
        label: Seeders
      - date
`;

    if (session.authMode === 'api_key') {
      const unauthorized = await publicApi.POST('/v1/indexers/definitions/import/cardigann', {
        body: { yaml_payload: yamlPayload, is_deprecated: false },
      });
      expect(unauthorized.response.status).toBe(401);
    }

    const imported = await api.POST('/v1/indexers/definitions/import/cardigann', {
      body: { yaml_payload: yamlPayload, is_deprecated: false },
    });
    expect(imported.response.status).toBe(201);
    expect(imported.data?.definition.upstream_source).toBe('cardigann');
    expect(imported.data?.definition.upstream_slug).toBe(slug);
    expect(imported.data?.field_count).toBe(2);
    expect(imported.data?.option_count).toBe(2);

    const definitions = await api.GET('/v1/indexers/definitions');
    expect(definitions.response.status).toBe(200);
    expect(
      definitions.data?.definitions.some(
        (definition) =>
          definition.upstream_source === 'cardigann' && definition.upstream_slug === slug
      )
    ).toBeTruthy();
  });
});
