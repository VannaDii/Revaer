-- Stored procedures for rate limit policies and token buckets.

CREATE OR REPLACE FUNCTION rate_limit_policy_create_v1(
    actor_user_public_id UUID,
    display_name_input VARCHAR,
    rpm_input INTEGER,
    burst_input INTEGER,
    concurrent_input INTEGER
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to create rate limit policy';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    trimmed_display_name VARCHAR(256);
    new_policy_id BIGINT;
    new_policy_public_id UUID;
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

    IF display_name_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'display_name_missing';
    END IF;

    trimmed_display_name := trim(display_name_input);

    IF trimmed_display_name = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'display_name_empty';
    END IF;

    IF char_length(trimmed_display_name) > 256 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'display_name_too_long';
    END IF;

    IF rpm_input IS NULL OR burst_input IS NULL OR concurrent_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'limit_missing';
    END IF;

    IF rpm_input < 1 OR rpm_input > 6000 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'rpm_out_of_range';
    END IF;

    IF burst_input < 0 OR burst_input > 6000 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'burst_out_of_range';
    END IF;

    IF concurrent_input < 1 OR concurrent_input > 64 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'concurrent_out_of_range';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM rate_limit_policy
        WHERE display_name = trimmed_display_name
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'display_name_already_exists';
    END IF;

    new_policy_public_id := gen_random_uuid();

    INSERT INTO rate_limit_policy (
        rate_limit_policy_public_id,
        display_name,
        requests_per_minute,
        burst,
        concurrent_requests,
        created_at,
        updated_at
    )
    VALUES (
        new_policy_public_id,
        trimmed_display_name,
        rpm_input,
        burst_input,
        concurrent_input,
        now(),
        now()
    )
    RETURNING rate_limit_policy_id INTO new_policy_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'rate_limit_policy',
        new_policy_id,
        new_policy_public_id,
        'create',
        actor_user_id,
        'rate_limit_policy_create'
    );

    RETURN new_policy_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION rate_limit_policy_create(
    actor_user_public_id UUID,
    display_name_input VARCHAR,
    rpm_input INTEGER,
    burst_input INTEGER,
    concurrent_input INTEGER
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN rate_limit_policy_create_v1(
        actor_user_public_id,
        display_name_input,
        rpm_input,
        burst_input,
        concurrent_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION rate_limit_policy_update_v1(
    actor_user_public_id UUID,
    rate_limit_policy_public_id_input UUID,
    display_name_input VARCHAR,
    rpm_input INTEGER,
    burst_input INTEGER,
    concurrent_input INTEGER
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to update rate limit policy';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    policy_id BIGINT;
    is_system_policy BOOLEAN;
    trimmed_display_name VARCHAR(256);
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

    IF rate_limit_policy_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_missing';
    END IF;

    SELECT rate_limit_policy_id, is_system
    INTO policy_id, is_system_policy
    FROM rate_limit_policy
    WHERE rate_limit_policy_public_id = rate_limit_policy_public_id_input;

    IF policy_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_not_found';
    END IF;

    IF is_system_policy THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_is_system';
    END IF;

    IF display_name_input IS NOT NULL THEN
        trimmed_display_name := trim(display_name_input);

        IF trimmed_display_name = '' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'display_name_empty';
        END IF;

        IF char_length(trimmed_display_name) > 256 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'display_name_too_long';
        END IF;

        IF EXISTS (
            SELECT 1
            FROM rate_limit_policy
            WHERE display_name = trimmed_display_name
              AND rate_limit_policy_id <> policy_id
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'display_name_already_exists';
        END IF;
    END IF;

    IF rpm_input IS NOT NULL AND (rpm_input < 1 OR rpm_input > 6000) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'rpm_out_of_range';
    END IF;

    IF burst_input IS NOT NULL AND (burst_input < 0 OR burst_input > 6000) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'burst_out_of_range';
    END IF;

    IF concurrent_input IS NOT NULL AND (concurrent_input < 1 OR concurrent_input > 64) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'concurrent_out_of_range';
    END IF;

    UPDATE rate_limit_policy
    SET display_name = COALESCE(trimmed_display_name, display_name),
        requests_per_minute = COALESCE(rpm_input, requests_per_minute),
        burst = COALESCE(burst_input, burst),
        concurrent_requests = COALESCE(concurrent_input, concurrent_requests),
        updated_at = now()
    WHERE rate_limit_policy_id = policy_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'rate_limit_policy',
        policy_id,
        rate_limit_policy_public_id_input,
        'update',
        actor_user_id,
        'rate_limit_policy_update'
    );
END;
$$;

CREATE OR REPLACE FUNCTION rate_limit_policy_update(
    actor_user_public_id UUID,
    rate_limit_policy_public_id_input UUID,
    display_name_input VARCHAR,
    rpm_input INTEGER,
    burst_input INTEGER,
    concurrent_input INTEGER
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM rate_limit_policy_update_v1(
        actor_user_public_id,
        rate_limit_policy_public_id_input,
        display_name_input,
        rpm_input,
        burst_input,
        concurrent_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION rate_limit_policy_soft_delete_v1(
    actor_user_public_id UUID,
    rate_limit_policy_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to delete rate limit policy';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    policy_id BIGINT;
    is_system_policy BOOLEAN;
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

    IF rate_limit_policy_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_missing';
    END IF;

    SELECT rate_limit_policy_id, is_system
    INTO policy_id, is_system_policy
    FROM rate_limit_policy
    WHERE rate_limit_policy_public_id = rate_limit_policy_public_id_input;

    IF policy_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_not_found';
    END IF;

    IF is_system_policy THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_is_system';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM indexer_instance_rate_limit
        WHERE rate_limit_policy_id = policy_id
    ) OR EXISTS (
        SELECT 1
        FROM routing_policy_rate_limit
        WHERE rate_limit_policy_id = policy_id
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_in_use';
    END IF;

    UPDATE rate_limit_policy
    SET deleted_at = now(),
        updated_at = now()
    WHERE rate_limit_policy_id = policy_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'rate_limit_policy',
        policy_id,
        rate_limit_policy_public_id_input,
        'soft_delete',
        actor_user_id,
        'rate_limit_policy_soft_delete'
    );
END;
$$;

CREATE OR REPLACE FUNCTION rate_limit_policy_soft_delete(
    actor_user_public_id UUID,
    rate_limit_policy_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM rate_limit_policy_soft_delete_v1(
        actor_user_public_id,
        rate_limit_policy_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_instance_set_rate_limit_policy_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    rate_limit_policy_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to set indexer rate limit policy';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    indexer_id BIGINT;
    indexer_deleted_at TIMESTAMPTZ;
    policy_id BIGINT;
    policy_deleted_at TIMESTAMPTZ;
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

    SELECT indexer_instance_id, deleted_at
    INTO indexer_id, indexer_deleted_at
    FROM indexer_instance
    WHERE indexer_instance_public_id = indexer_instance_public_id_input;

    IF indexer_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_not_found';
    END IF;

    IF indexer_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_deleted';
    END IF;

    IF rate_limit_policy_public_id_input IS NULL THEN
        DELETE FROM indexer_instance_rate_limit
        WHERE indexer_instance_id = indexer_id;
    ELSE
        SELECT rate_limit_policy_id, deleted_at
        INTO policy_id, policy_deleted_at
        FROM rate_limit_policy
        WHERE rate_limit_policy_public_id = rate_limit_policy_public_id_input;

        IF policy_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'policy_not_found';
        END IF;

        IF policy_deleted_at IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'policy_deleted';
        END IF;

        INSERT INTO indexer_instance_rate_limit (
            indexer_instance_id,
            rate_limit_policy_id
        )
        VALUES (
            indexer_id,
            policy_id
        )
        ON CONFLICT (indexer_instance_id)
        DO UPDATE SET rate_limit_policy_id = EXCLUDED.rate_limit_policy_id;
    END IF;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'indexer_instance',
        indexer_id,
        indexer_instance_public_id_input,
        'update',
        actor_user_id,
        'indexer_rate_limit_set'
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_instance_set_rate_limit_policy(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    rate_limit_policy_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_instance_set_rate_limit_policy_v1(
        actor_user_public_id,
        indexer_instance_public_id_input,
        rate_limit_policy_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION routing_policy_set_rate_limit_policy_v1(
    actor_user_public_id UUID,
    routing_policy_public_id_input UUID,
    rate_limit_policy_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to set routing policy rate limit';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    policy_id BIGINT;
    policy_deleted_at TIMESTAMPTZ;
    rate_policy_id BIGINT;
    rate_policy_deleted_at TIMESTAMPTZ;
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

    IF routing_policy_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'routing_policy_missing';
    END IF;

    SELECT routing_policy_id, deleted_at
    INTO policy_id, policy_deleted_at
    FROM routing_policy
    WHERE routing_policy_public_id = routing_policy_public_id_input;

    IF policy_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'routing_policy_not_found';
    END IF;

    IF policy_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'routing_policy_deleted';
    END IF;

    IF rate_limit_policy_public_id_input IS NULL THEN
        DELETE FROM routing_policy_rate_limit
        WHERE routing_policy_id = policy_id;
    ELSE
        SELECT rate_limit_policy_id, deleted_at
        INTO rate_policy_id, rate_policy_deleted_at
        FROM rate_limit_policy
        WHERE rate_limit_policy_public_id = rate_limit_policy_public_id_input;

        IF rate_policy_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'policy_not_found';
        END IF;

        IF rate_policy_deleted_at IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'policy_deleted';
        END IF;

        INSERT INTO routing_policy_rate_limit (
            routing_policy_id,
            rate_limit_policy_id
        )
        VALUES (
            policy_id,
            rate_policy_id
        )
        ON CONFLICT (routing_policy_id)
        DO UPDATE SET rate_limit_policy_id = EXCLUDED.rate_limit_policy_id;
    END IF;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'routing_policy',
        policy_id,
        routing_policy_public_id_input,
        'update',
        actor_user_id,
        'routing_policy_rate_limit_set'
    );
END;
$$;

CREATE OR REPLACE FUNCTION routing_policy_set_rate_limit_policy(
    actor_user_public_id UUID,
    routing_policy_public_id_input UUID,
    rate_limit_policy_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM routing_policy_set_rate_limit_policy_v1(
        actor_user_public_id,
        routing_policy_public_id_input,
        rate_limit_policy_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION rate_limit_try_consume_v1(
    scope_type_input rate_limit_scope,
    scope_id_input BIGINT,
    capacity_input INTEGER,
    tokens_input INTEGER DEFAULT 1
)
RETURNS TABLE(allowed BOOLEAN, tokens_used INTEGER)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to consume rate limit tokens';
    errcode CONSTANT text := 'P0001';
    window_start_value TIMESTAMPTZ;
    existing_tokens INTEGER;
BEGIN
    IF scope_type_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'scope_missing';
    END IF;

    IF scope_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'scope_id_missing';
    END IF;

    IF capacity_input IS NULL OR capacity_input < 1 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'capacity_invalid';
    END IF;

    IF tokens_input IS NULL THEN
        tokens_input := 1;
    END IF;

    IF tokens_input < 1 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tokens_invalid';
    END IF;

    window_start_value := date_trunc('minute', now() AT TIME ZONE 'UTC');

    INSERT INTO rate_limit_state (
        scope_type,
        scope_id,
        window_start,
        tokens_used
    )
    VALUES (
        scope_type_input,
        scope_id_input,
        window_start_value,
        0
    )
    ON CONFLICT (scope_type, scope_id, window_start)
    DO NOTHING;

    SELECT tokens_used
    INTO existing_tokens
    FROM rate_limit_state
    WHERE scope_type = scope_type_input
      AND scope_id = scope_id_input
      AND window_start = window_start_value
    FOR UPDATE;

    IF existing_tokens + tokens_input <= capacity_input THEN
        existing_tokens := existing_tokens + tokens_input;

        UPDATE rate_limit_state
        SET tokens_used = existing_tokens,
            updated_at = now()
        WHERE scope_type = scope_type_input
          AND scope_id = scope_id_input
          AND window_start = window_start_value;

        allowed := TRUE;
        tokens_used := existing_tokens;
    ELSE
        allowed := FALSE;
        tokens_used := existing_tokens;
    END IF;

    RETURN NEXT;
END;
$$;
