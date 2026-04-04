-- Fix variable/column name conflicts in stored procedures.

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

    SELECT rate_limit_state.tokens_used
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

CREATE OR REPLACE FUNCTION indexer_instance_test_prepare_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID
)
RETURNS TABLE (
    can_execute BOOLEAN,
    error_class error_class,
    error_code VARCHAR(64),
    detail VARCHAR(256),
    engine engine,
    routing_policy_public_id UUID,
    connect_timeout_ms INTEGER,
    read_timeout_ms INTEGER,
    field_names VARCHAR[],
    field_types field_type[],
    value_plain VARCHAR[],
    value_int INTEGER[],
    value_decimal NUMERIC[],
    value_bool BOOLEAN[],
    secret_public_ids UUID[]
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to prepare indexer test';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    migration_state_value indexer_instance_migration_state;
    routing_policy_id_value BIGINT;
    routing_policy_deleted_at TIMESTAMPTZ;
    definition_id BIGINT;
    missing_fields TEXT;
    missing_detail VARCHAR(256);
    missing_count INTEGER;
BEGIN
    IF actor_user_public_id IS NOT NULL THEN
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
    END IF;

    IF indexer_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_missing';
    END IF;

    SELECT
        indexer_instance.indexer_instance_id,
        indexer_instance.deleted_at,
        indexer_instance.migration_state,
        indexer_instance.routing_policy_id,
        indexer_instance.connect_timeout_ms,
        indexer_instance.read_timeout_ms,
        indexer_instance.indexer_definition_id
    INTO
        instance_id,
        instance_deleted_at,
        migration_state_value,
        routing_policy_id_value,
        connect_timeout_ms,
        read_timeout_ms,
        definition_id
    FROM indexer_instance
    WHERE indexer_instance_public_id = indexer_instance_public_id_input;

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

    SELECT indexer_definition.engine
    INTO engine
    FROM indexer_definition
    WHERE indexer_definition_id = definition_id;

    IF engine IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'definition_not_found';
    END IF;

    routing_policy_public_id := NULL;
    IF routing_policy_id_value IS NOT NULL THEN
        SELECT routing_policy.routing_policy_public_id, routing_policy.deleted_at
        INTO routing_policy_public_id, routing_policy_deleted_at
        FROM routing_policy
        WHERE routing_policy_id = routing_policy_id_value;

        IF routing_policy_public_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'routing_policy_not_found';
        END IF;

        IF routing_policy_deleted_at IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'routing_policy_deleted';
        END IF;
    END IF;

    SELECT
        string_agg(required.name, ', ' ORDER BY required.name),
        count(*)
    INTO missing_fields, missing_count
    FROM (
        SELECT name
        FROM indexer_definition_field
        WHERE indexer_definition_id = definition_id
          AND is_required = TRUE
          AND field_type IN ('password', 'api_key', 'cookie', 'token', 'header_value')
    ) AS required
    LEFT JOIN (
        SELECT fv.field_name
        FROM indexer_instance_field_value fv
        JOIN secret_binding sb
            ON sb.bound_table = 'indexer_instance_field_value'
            AND sb.bound_id = fv.indexer_instance_field_value_id
        JOIN secret s
            ON s.secret_id = sb.secret_id
            AND s.is_revoked = FALSE
        WHERE fv.indexer_instance_id = instance_id
    ) AS bound
        ON bound.field_name = required.name
    WHERE bound.field_name IS NULL;

    IF missing_count > 0 THEN
        missing_detail := left(missing_fields, 256);

        can_execute := FALSE;
        error_class := 'auth_error';
        error_code := 'missing_secret';
        detail := missing_detail;
        field_names := NULL;
        field_types := NULL;
        value_plain := NULL;
        value_int := NULL;
        value_decimal := NULL;
        value_bool := NULL;
        secret_public_ids := NULL;

        IF migration_state_value IS NOT NULL THEN
            UPDATE indexer_instance
            SET migration_state = 'needs_secret',
                is_enabled = FALSE
            WHERE indexer_instance_id = instance_id;
        END IF;

        RETURN NEXT;
        RETURN;
    END IF;

    SELECT
        array_agg(fv.field_name ORDER BY fv.field_name),
        array_agg(fv.field_type ORDER BY fv.field_name),
        array_agg(fv.value_plain ORDER BY fv.field_name),
        array_agg(fv.value_int ORDER BY fv.field_name),
        array_agg(fv.value_decimal ORDER BY fv.field_name),
        array_agg(fv.value_bool ORDER BY fv.field_name),
        array_agg(s.secret_public_id ORDER BY fv.field_name)
    INTO
        field_names,
        field_types,
        value_plain,
        value_int,
        value_decimal,
        value_bool,
        secret_public_ids
    FROM indexer_instance_field_value fv
    LEFT JOIN secret_binding sb
        ON sb.bound_table = 'indexer_instance_field_value'
        AND sb.bound_id = fv.indexer_instance_field_value_id
    LEFT JOIN secret s
        ON s.secret_id = sb.secret_id
        AND s.is_revoked = FALSE
    WHERE fv.indexer_instance_id = instance_id;

    can_execute := TRUE;
    error_class := NULL;
    error_code := NULL;
    detail := NULL;

    RETURN NEXT;
    RETURN;
END;
$$;
