import { test, expect } from '../../fixtures/api';

test.describe('Indexer health notifications', () => {
  test('covers notification hook endpoints', async ({ api, publicApi, session }) => {
    if (session.authMode === 'api_key') {
      const unauthorized = await publicApi.GET('/v1/indexers/health-notifications');
      expect(unauthorized.response.status).toBe(401);
    }

    const create = await api.POST('/v1/indexers/health-notifications', {
      body: {
        channel: 'webhook',
        display_name: `E2E Hook ${Date.now()}`,
        status_threshold: 'failing',
        webhook_url: 'https://hooks.example.test/indexers',
      },
    });
    expect(create.response.status).toBe(201);
    expect(create.data?.channel).toBe('webhook');
    expect(create.data?.webhook_url).toBe('https://hooks.example.test/indexers');

    const hookId = create.data?.indexer_health_notification_hook_public_id;
    expect(hookId).toBeTruthy();

    const list = await api.GET('/v1/indexers/health-notifications');
    expect(list.response.status).toBe(200);
    expect(
      list.data?.hooks.some(
        (hook) => hook.indexer_health_notification_hook_public_id === hookId,
      ),
    ).toBeTruthy();

    const update = await api.PATCH('/v1/indexers/health-notifications', {
      body: {
        indexer_health_notification_hook_public_id: hookId!,
        display_name: 'E2E Hook Updated',
        status_threshold: 'quarantined',
        webhook_url: 'https://hooks.example.test/escalation',
        is_enabled: false,
      },
    });
    expect(update.response.status).toBe(200);
    expect(update.data?.display_name).toBe('E2E Hook Updated');
    expect(update.data?.status_threshold).toBe('quarantined');
    expect(update.data?.is_enabled).toBe(false);

    const remove = await api.DELETE('/v1/indexers/health-notifications', {
      body: { indexer_health_notification_hook_public_id: hookId! },
    });
    expect(remove.response.status).toBe(204);
  });
});
