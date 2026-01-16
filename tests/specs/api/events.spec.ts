import { test, expect } from '@playwright/test';
import { authHeaders } from '../../support/headers';
import { loadSession, type ApiSession } from '../../support/session';

let session: ApiSession;
const apiBaseUrl = process.env.E2E_API_BASE_URL ?? 'http://localhost:7070';
const STREAM_TIMEOUT_MS = 5000;
const CHUNK_TIMEOUT_MS = 3000;

test.beforeAll(() => {
  session = loadSession();
});

async function openEventStream(path: string, expectChunk: boolean): Promise<void> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), STREAM_TIMEOUT_MS);
  try {
    const response = await fetch(`${apiBaseUrl}${path}`, {
      headers: {
        ...authHeaders(session),
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
  test('streams logs', async () => {
    await openEventStream('/v1/logs/stream', false);
  });

  test('streams events', async () => {
    await openEventStream('/v1/events', true);
  });

  test('streams events on explicit endpoint', async () => {
    await openEventStream('/v1/events/stream', true);
  });

  test('streams torrent events', async () => {
    await openEventStream('/v1/torrents/events', true);
  });
});
