import { randomUUID } from 'node:crypto';
import { test, expect } from '../../fixtures/api';

test.describe('Indexer search requests', () => {
  test('creates and cancels search requests', async ({ api, publicApi, session }) => {
    if (session.authMode === 'api_key') {
      const unauthorized = await publicApi.POST('/v1/indexers/search-requests', {
        body: { query_text: 'Dune', query_type: 'free_text' },
      });
      expect(unauthorized.response.status).toBe(401);
    }

    const invalidCreate = await api.POST('/v1/indexers/search-requests', {
      body: {
        query_text: 'Dune',
        query_type: '   ',
      },
    });
    expect(invalidCreate.response.status).toBe(400);

    const created = await api.POST('/v1/indexers/search-requests', {
      body: {
        query_text: 'Dune',
        query_type: 'free_text',
      },
    });
    expect(created.response.status).toBe(201);
    if (!created.data) {
      throw new Error('expected search request creation response');
    }
    const requestId = created.data.search_request_public_id;

    const pages = await api.GET(
      '/v1/indexers/search-requests/{search_request_public_id}/pages',
      {
        params: { path: { search_request_public_id: requestId } },
      }
    );
    expect(pages.response.status).toBe(200);
    if (!pages.data) {
      throw new Error('expected search page list response');
    }
    const pageNumber = pages.data.pages[0]?.page_number ?? 1;

    const page = await api.GET(
      '/v1/indexers/search-requests/{search_request_public_id}/pages/{page_number}',
      {
        params: {
          path: {
            search_request_public_id: requestId,
            page_number: pageNumber,
          },
        },
      }
    );
    expect(page.response.status).toBe(200);

    if (session.authMode === 'api_key') {
      const unauthorizedCancel = await publicApi.POST(
        '/v1/indexers/search-requests/{search_request_public_id}/cancel',
        {
          params: { path: { search_request_public_id: randomUUID() } },
        }
      );
      expect(unauthorizedCancel.response.status).toBe(401);
    }

    const cancel = await api.POST(
      '/v1/indexers/search-requests/{search_request_public_id}/cancel',
      {
        params: { path: { search_request_public_id: randomUUID() } },
      }
    );
    expect(cancel.response.status).toBe(404);
  });
});
