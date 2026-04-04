-- Direct read proc for a single health notification hook.

CREATE OR REPLACE FUNCTION indexer_health_notification_hook_get_v1(
    actor_user_public_id UUID,
    indexer_health_notification_hook_public_id_input UUID
)
RETURNS TABLE (
    indexer_health_notification_hook_public_id UUID,
    channel indexer_health_notification_channel,
    display_name VARCHAR,
    status_threshold indexer_health_notification_threshold,
    webhook_url VARCHAR,
    email VARCHAR,
    is_enabled BOOLEAN,
    updated_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to get health notification hook';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_missing';
    END IF;

    SELECT user_id, role
    INTO actor_user_id, actor_role
    FROM app_user
    WHERE user_public_id = actor_user_public_id;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_not_found';
    END IF;

    IF actor_role NOT IN ('owner', 'admin') THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_unauthorized';
    END IF;

    IF indexer_health_notification_hook_public_id_input IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'hook_missing';
    END IF;

    RETURN QUERY
    SELECT
        hook.indexer_health_notification_hook_public_id,
        hook.channel,
        hook.display_name,
        hook.status_threshold,
        hook.webhook_url,
        hook.email,
        hook.is_enabled,
        hook.updated_at
    FROM indexer_health_notification_hook hook
    WHERE hook.indexer_health_notification_hook_public_id =
        indexer_health_notification_hook_public_id_input;

    IF NOT FOUND THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'hook_not_found';
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_health_notification_hook_get(
    actor_user_public_id UUID,
    indexer_health_notification_hook_public_id_input UUID
)
RETURNS TABLE (
    indexer_health_notification_hook_public_id UUID,
    channel indexer_health_notification_channel,
    display_name VARCHAR,
    status_threshold indexer_health_notification_threshold,
    webhook_url VARCHAR,
    email VARCHAR,
    is_enabled BOOLEAN,
    updated_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT *
    FROM indexer_health_notification_hook_get_v1(
        actor_user_public_id,
        indexer_health_notification_hook_public_id_input
    );
END;
$$;
