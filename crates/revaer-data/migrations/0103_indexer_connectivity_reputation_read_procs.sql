-- Read procedures for operator-facing connectivity and reputation views.

CREATE OR REPLACE FUNCTION indexer_connectivity_profile_get_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID
)
RETURNS TABLE (
    profile_exists BOOLEAN,
    status connectivity_status,
    error_class error_class,
    latency_p50_ms INTEGER,
    latency_p95_ms INTEGER,
    success_rate_1h NUMERIC(5, 4),
    success_rate_24h NUMERIC(5, 4),
    last_checked_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to fetch connectivity profile';
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
        profile.indexer_instance_id IS NOT NULL,
        profile.status,
        profile.error_class,
        profile.latency_p50_ms,
        profile.latency_p95_ms,
        profile.success_rate_1h,
        profile.success_rate_24h,
        profile.last_checked_at
    FROM indexer_instance inst
    LEFT JOIN indexer_connectivity_profile profile
        ON profile.indexer_instance_id = inst.indexer_instance_id
    WHERE inst.indexer_instance_id = instance_id;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_source_reputation_list_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    window_key_input reputation_window,
    limit_input INTEGER
)
RETURNS TABLE (
    window_key reputation_window,
    window_start TIMESTAMPTZ,
    request_success_rate NUMERIC(5, 4),
    acquisition_success_rate NUMERIC(5, 4),
    fake_rate NUMERIC(5, 4),
    dmca_rate NUMERIC(5, 4),
    request_count INTEGER,
    request_success_count INTEGER,
    acquisition_count INTEGER,
    acquisition_success_count INTEGER,
    min_samples INTEGER,
    computed_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to list source reputation';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    reputation_limit INTEGER;
    resolved_window reputation_window;
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

    resolved_window := COALESCE(window_key_input, '1h'::reputation_window);
    reputation_limit := COALESCE(limit_input, 10);
    IF reputation_limit < 1 OR reputation_limit > 100 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'limit_out_of_range';
    END IF;

    RETURN QUERY
    SELECT
        reputation.window_key,
        reputation.window_start,
        reputation.request_success_rate,
        reputation.acquisition_success_rate,
        reputation.fake_rate,
        reputation.dmca_rate,
        reputation.request_count,
        reputation.request_success_count,
        reputation.acquisition_count,
        reputation.acquisition_success_count,
        reputation.min_samples,
        reputation.computed_at
    FROM source_reputation reputation
    WHERE reputation.indexer_instance_id = instance_id
      AND reputation.window_key = resolved_window
    ORDER BY reputation.window_start DESC
    LIMIT reputation_limit;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_connectivity_profile_get(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID
)
RETURNS TABLE (
    profile_exists BOOLEAN,
    status connectivity_status,
    error_class error_class,
    latency_p50_ms INTEGER,
    latency_p95_ms INTEGER,
    success_rate_1h NUMERIC(5, 4),
    success_rate_24h NUMERIC(5, 4),
    last_checked_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT *
    FROM indexer_connectivity_profile_get_v1(
        actor_user_public_id,
        indexer_instance_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_source_reputation_list(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    window_key_input reputation_window,
    limit_input INTEGER
)
RETURNS TABLE (
    window_key reputation_window,
    window_start TIMESTAMPTZ,
    request_success_rate NUMERIC(5, 4),
    acquisition_success_rate NUMERIC(5, 4),
    fake_rate NUMERIC(5, 4),
    dmca_rate NUMERIC(5, 4),
    request_count INTEGER,
    request_success_count INTEGER,
    acquisition_count INTEGER,
    acquisition_success_count INTEGER,
    min_samples INTEGER,
    computed_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT *
    FROM indexer_source_reputation_list_v1(
        actor_user_public_id,
        indexer_instance_public_id_input,
        window_key_input,
        limit_input
    );
END;
$$;
