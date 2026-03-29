-- Read procedure for operator-facing indexer health-event drill-down.

CREATE OR REPLACE FUNCTION indexer_health_event_list_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    limit_input INTEGER
)
RETURNS TABLE (
    occurred_at TIMESTAMPTZ,
    event_type health_event_type,
    latency_ms INTEGER,
    http_status INTEGER,
    error_class error_class,
    detail VARCHAR
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to list health events';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    event_limit INTEGER;
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

    event_limit := COALESCE(limit_input, 20);
    IF event_limit < 1 OR event_limit > 100 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'limit_out_of_range';
    END IF;

    RETURN QUERY
    SELECT
        health.occurred_at,
        health.event_type,
        health.latency_ms,
        health.http_status,
        health.error_class,
        health.detail
    FROM indexer_health_event health
    WHERE health.indexer_instance_id = instance_id
    ORDER BY health.occurred_at DESC, health.indexer_health_event_id DESC
    LIMIT event_limit;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_health_event_list(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    limit_input INTEGER
)
RETURNS TABLE (
    occurred_at TIMESTAMPTZ,
    event_type health_event_type,
    latency_ms INTEGER,
    http_status INTEGER,
    error_class error_class,
    detail VARCHAR
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT *
    FROM indexer_health_event_list_v1(
        actor_user_public_id,
        indexer_instance_public_id_input,
        limit_input
    );
END;
$$;
