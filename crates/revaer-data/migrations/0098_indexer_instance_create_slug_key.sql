-- Switch indexer instance creation to the public upstream slug key.

DROP FUNCTION IF EXISTS indexer_instance_create_v1(
    UUID,
    BIGINT,
    VARCHAR,
    INTEGER,
    trust_tier_key,
    UUID
);

DROP FUNCTION IF EXISTS indexer_instance_create(
    UUID,
    BIGINT,
    VARCHAR,
    INTEGER,
    trust_tier_key,
    UUID
);

CREATE OR REPLACE FUNCTION indexer_instance_create_v1(
    actor_user_public_id UUID,
    indexer_definition_upstream_slug_input VARCHAR,
    display_name_input VARCHAR,
    priority_input INTEGER,
    trust_tier_key_input trust_tier_key,
    routing_policy_public_id_input UUID
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to create indexer instance';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    definition_id BIGINT;
    definition_protocol protocol;
    definition_deprecated BOOLEAN;
    routing_policy_id_value BIGINT;
    routing_policy_deleted_at TIMESTAMPTZ;
    new_instance_id BIGINT;
    new_instance_public_id UUID;
    trimmed_display_name VARCHAR(256);
    trimmed_definition_upstream_slug VARCHAR(128);
    resolved_priority INTEGER;
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

    IF indexer_definition_upstream_slug_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'definition_missing';
    END IF;

    trimmed_definition_upstream_slug := trim(indexer_definition_upstream_slug_input);

    IF trimmed_definition_upstream_slug = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'definition_missing';
    END IF;

    SELECT indexer_definition_id, protocol, is_deprecated
    INTO definition_id, definition_protocol, definition_deprecated
    FROM indexer_definition
    WHERE upstream_slug = trimmed_definition_upstream_slug;

    IF definition_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'definition_not_found';
    END IF;

    IF definition_deprecated THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'definition_deprecated';
    END IF;

    IF definition_protocol <> 'torrent' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'unsupported_protocol';
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

    IF EXISTS (
        SELECT 1
        FROM indexer_instance
        WHERE display_name = trimmed_display_name
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'display_name_already_exists';
    END IF;

    resolved_priority := COALESCE(priority_input, 50);

    IF resolved_priority < 0 OR resolved_priority > 100 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'priority_out_of_range';
    END IF;

    IF routing_policy_public_id_input IS NOT NULL THEN
        SELECT routing_policy_id, deleted_at
        INTO routing_policy_id_value, routing_policy_deleted_at
        FROM routing_policy
        WHERE routing_policy_public_id = routing_policy_public_id_input;

        IF routing_policy_id_value IS NULL THEN
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
    ELSE
        routing_policy_id_value := NULL;
    END IF;

    IF trust_tier_key_input IS NOT NULL THEN
        IF NOT EXISTS (
            SELECT 1
            FROM trust_tier
            WHERE trust_tier_key = trust_tier_key_input
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'trust_tier_not_found';
        END IF;
    END IF;

    new_instance_public_id := gen_random_uuid();

    INSERT INTO indexer_instance (
        indexer_instance_public_id,
        indexer_definition_id,
        display_name,
        is_enabled,
        priority,
        trust_tier_key,
        routing_policy_id,
        created_by_user_id,
        updated_by_user_id
    )
    VALUES (
        new_instance_public_id,
        definition_id,
        trimmed_display_name,
        TRUE,
        resolved_priority,
        trust_tier_key_input,
        routing_policy_id_value,
        actor_user_id,
        actor_user_id
    )
    RETURNING indexer_instance_id INTO new_instance_id;

    INSERT INTO indexer_cf_state (
        indexer_instance_id,
        state,
        last_changed_at,
        cf_session_id,
        cf_session_expires_at,
        cooldown_until,
        backoff_seconds,
        consecutive_failures,
        last_error_class
    )
    VALUES (
        new_instance_id,
        'clear',
        now(),
        NULL,
        NULL,
        NULL,
        NULL,
        0,
        NULL
    );

    INSERT INTO indexer_rss_subscription (
        indexer_instance_id,
        is_enabled,
        interval_seconds,
        last_polled_at,
        next_poll_at,
        backoff_seconds,
        last_error_class
    )
    VALUES (
        new_instance_id,
        TRUE,
        900,
        NULL,
        now() + make_interval(secs => random_jitter_seconds(60)),
        NULL,
        NULL
    );

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
        new_instance_id,
        new_instance_public_id,
        'create',
        actor_user_id,
        'indexer_instance_create'
    );

    RETURN new_instance_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_instance_create(
    actor_user_public_id UUID,
    indexer_definition_upstream_slug_input VARCHAR,
    display_name_input VARCHAR,
    priority_input INTEGER,
    trust_tier_key_input trust_tier_key,
    routing_policy_public_id_input UUID
)
RETURNS UUID
LANGUAGE sql
AS $$
    SELECT indexer_instance_create_v1(
        actor_user_public_id => actor_user_public_id,
        indexer_definition_upstream_slug_input => indexer_definition_upstream_slug_input,
        display_name_input => display_name_input,
        priority_input => priority_input,
        trust_tier_key_input => trust_tier_key_input,
        routing_policy_public_id_input => routing_policy_public_id_input
    );
$$;
