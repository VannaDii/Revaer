import { test, expect } from '../../fixtures/api';
import { recordApiCoverage } from '../../support/api/coverage';
import { authHeaders } from '../../support/headers';
const apiBaseUrl = process.env.E2E_API_BASE_URL ?? 'http://localhost:7070';
const STREAM_TIMEOUT_MS = 5000;
const CHUNK_TIMEOUT_MS = 3000;

async function openEventStream(
  path: string,
  expectChunk: boolean,
  headers: Record<string, string>,
): Promise<void> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), STREAM_TIMEOUT_MS);
  try {
    recordApiCoverage('GET', path);
    const response = await fetch(`${apiBaseUrl}${path}`, {
      headers: {
        ...headers,
        accept: 'text/event-stream',
      },
      signal: controller.signal,
    });
    expect(response.ok).toBeTruthy();
    const contentType = response.headers.get('content-type') ?? '';
    expect(contentType).toContain('text/event-stream');
    if (expectChunk && response.body) {
      const reader = response.body.getReader();
      const result = await Promise.race([
        reader.read(),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error('Timed out waiting for SSE data')), CHUNK_TIMEOUT_MS),
        ),
      ]);
      expect(result.value?.length ?? 0).toBeGreaterThan(0);
      await reader.cancel();
    }
  } finally {
    clearTimeout(timeout);
    controller.abort();
  }
}

test.describe('Event streams', () => {
  test('streams logs', async ({ session }) => {
    await openEventStream('/v1/logs/stream', false, authHeaders(session));
  });

  test('streams events', async ({ session }) => {
    await openEventStream('/v1/events', true, authHeaders(session));
  });

  test('streams events on explicit endpoint', async ({ session }) => {
    await openEventStream('/v1/events/stream', true, authHeaders(session));
  });

  test('streams torrent events', async ({ session }) => {
    await openEventStream('/v1/torrents/events', true, authHeaders(session));
  });
});
