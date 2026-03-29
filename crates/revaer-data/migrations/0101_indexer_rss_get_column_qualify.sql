-- Qualify indexer_rss_subscription_get_v1 lookup columns to avoid PL/pgSQL ambiguity.

CREATE OR REPLACE FUNCTION indexer_rss_subscription_get_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID
)
RETURNS TABLE (
    indexer_instance_public_id UUID,
    instance_is_enabled BOOLEAN,
    instance_enable_rss BOOLEAN,
    subscription_exists BOOLEAN,
    subscription_is_enabled BOOLEAN,
    interval_seconds INTEGER,
    last_polled_at TIMESTAMPTZ,
    next_poll_at TIMESTAMPTZ,
    backoff_seconds INTEGER,
    last_error_class error_class
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to fetch RSS subscription';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_missing';
    END IF;

    SELECT user_id, role
    INTO actor_user_id, actor_role
    FROM app_user
    WHERE user_public_id = actor_user_public_id;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_not_found';
    END IF;

    IF actor_role NOT IN ('owner', 'admin') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_unauthorized';
    END IF;

    IF indexer_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_missing';
    END IF;

    SELECT inst.indexer_instance_id, inst.deleted_at
    INTO instance_id, instance_deleted_at
    FROM indexer_instance inst
    WHERE inst.indexer_instance_public_id = indexer_instance_public_id_input;

    IF instance_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_not_found';
    END IF;

    IF instance_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_deleted';
    END IF;

    RETURN QUERY
    SELECT
        inst.indexer_instance_public_id,
        inst.is_enabled,
        inst.enable_rss,
        sub.indexer_rss_subscription_id IS NOT NULL,
        COALESCE(sub.is_enabled, FALSE),
        COALESCE(sub.interval_seconds, 900),
        sub.last_polled_at,
        sub.next_poll_at,
        sub.backoff_seconds,
        sub.last_error_class
    FROM indexer_instance inst
    LEFT JOIN indexer_rss_subscription sub
        ON sub.indexer_instance_id = inst.indexer_instance_id
    WHERE inst.indexer_instance_id = instance_id;
END;
$$;
