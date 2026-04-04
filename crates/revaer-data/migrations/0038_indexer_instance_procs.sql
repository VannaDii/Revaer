-- Stored procedures for indexer instance management.

CREATE OR REPLACE FUNCTION indexer_instance_create_v1(
    actor_user_public_id UUID,
    indexer_definition_id_input BIGINT,
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
    definition_protocol protocol;
    definition_deprecated BOOLEAN;
    routing_policy_id_value BIGINT;
    routing_policy_deleted_at TIMESTAMPTZ;
    new_instance_id BIGINT;
    new_instance_public_id UUID;
    trimmed_display_name VARCHAR(256);
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

    IF indexer_definition_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'definition_missing';
    END IF;

    SELECT protocol, is_deprecated
    INTO definition_protocol, definition_deprecated
    FROM indexer_definition
    WHERE indexer_definition_id = indexer_definition_id_input;

    IF definition_protocol IS NULL THEN
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
        indexer_definition_id_input,
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
    indexer_definition_id_input BIGINT,
    display_name_input VARCHAR,
    priority_input INTEGER,
    trust_tier_key_input trust_tier_key,
    routing_policy_public_id_input UUID
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN indexer_instance_create_v1(
        actor_user_public_id,
        indexer_definition_id_input,
        display_name_input,
        priority_input,
        trust_tier_key_input,
        routing_policy_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_instance_update_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    display_name_input VARCHAR,
    priority_input INTEGER,
    trust_tier_key_input trust_tier_key,
    routing_policy_public_id_input UUID,
    is_enabled_input BOOLEAN,
    enable_rss_input BOOLEAN,
    enable_automatic_search_input BOOLEAN,
    enable_interactive_search_input BOOLEAN
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to update indexer instance';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    current_display_name VARCHAR(256);
    current_is_enabled BOOLEAN;
    current_enable_rss BOOLEAN;
    current_priority INTEGER;
    current_trust_tier trust_tier_key;
    current_routing_policy_id BIGINT;
    new_display_name VARCHAR(256);
    new_priority INTEGER;
    new_trust_tier trust_tier_key;
    new_routing_policy_id BIGINT;
    new_is_enabled BOOLEAN;
    new_enable_rss BOOLEAN;
    new_enable_automatic_search BOOLEAN;
    new_enable_interactive_search BOOLEAN;
    routing_policy_deleted_at TIMESTAMPTZ;
    audit_action audit_action;
    rss_row_exists BOOLEAN;
    rss_is_enabled BOOLEAN;
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

    SELECT
        indexer_instance_id,
        deleted_at,
        display_name,
        is_enabled,
        enable_rss,
        enable_automatic_search,
        enable_interactive_search,
        priority,
        trust_tier_key,
        routing_policy_id
    INTO
        instance_id,
        instance_deleted_at,
        current_display_name,
        current_is_enabled,
        current_enable_rss,
        new_enable_automatic_search,
        new_enable_interactive_search,
        current_priority,
        current_trust_tier,
        current_routing_policy_id
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

    IF display_name_input IS NOT NULL THEN
        new_display_name := trim(display_name_input);

        IF new_display_name = '' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'display_name_empty';
        END IF;

        IF char_length(new_display_name) > 256 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'display_name_too_long';
        END IF;

        IF EXISTS (
            SELECT 1
            FROM indexer_instance
            WHERE display_name = new_display_name
              AND indexer_instance_id <> instance_id
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'display_name_already_exists';
        END IF;
    ELSE
        new_display_name := current_display_name;
    END IF;

    IF priority_input IS NOT NULL THEN
        IF priority_input < 0 OR priority_input > 100 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'priority_out_of_range';
        END IF;
        new_priority := priority_input;
    ELSE
        new_priority := current_priority;
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
        new_trust_tier := trust_tier_key_input;
    ELSE
        new_trust_tier := current_trust_tier;
    END IF;

    IF routing_policy_public_id_input IS NOT NULL THEN
        SELECT routing_policy_id, deleted_at
        INTO new_routing_policy_id, routing_policy_deleted_at
        FROM routing_policy
        WHERE routing_policy_public_id = routing_policy_public_id_input;

        IF new_routing_policy_id IS NULL THEN
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
        new_routing_policy_id := current_routing_policy_id;
    END IF;

    new_is_enabled := COALESCE(is_enabled_input, current_is_enabled);
    new_enable_rss := COALESCE(enable_rss_input, current_enable_rss);
    new_enable_automatic_search := COALESCE(
        enable_automatic_search_input,
        new_enable_automatic_search
    );
    new_enable_interactive_search := COALESCE(
        enable_interactive_search_input,
        new_enable_interactive_search
    );

    UPDATE indexer_instance
    SET display_name = new_display_name,
        priority = new_priority,
        trust_tier_key = new_trust_tier,
        routing_policy_id = new_routing_policy_id,
        is_enabled = new_is_enabled,
        enable_rss = new_enable_rss,
        enable_automatic_search = new_enable_automatic_search,
        enable_interactive_search = new_enable_interactive_search,
        updated_by_user_id = actor_user_id,
        updated_at = now()
    WHERE indexer_instance_id = instance_id;

    SELECT EXISTS (
        SELECT 1
        FROM indexer_rss_subscription
        WHERE indexer_instance_id = instance_id
    ) INTO rss_row_exists;

    IF new_is_enabled IS FALSE OR new_enable_rss IS FALSE THEN
        IF rss_row_exists THEN
            UPDATE indexer_rss_subscription
            SET is_enabled = FALSE,
                next_poll_at = NULL
            WHERE indexer_instance_id = instance_id;
        ELSE
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
                instance_id,
                FALSE,
                900,
                NULL,
                NULL,
                NULL,
                NULL
            );
        END IF;
    ELSIF new_is_enabled IS TRUE AND new_enable_rss IS TRUE THEN
        IF rss_row_exists IS FALSE THEN
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
                instance_id,
                TRUE,
                900,
                NULL,
                now() + make_interval(secs => random_jitter_seconds(60)),
                NULL,
                NULL
            );
        END IF;
    END IF;

    IF current_is_enabled IS DISTINCT FROM new_is_enabled THEN
        IF new_is_enabled THEN
            audit_action := 'enable';
        ELSE
            audit_action := 'disable';
        END IF;
    ELSE
        audit_action := 'update';
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
        instance_id,
        indexer_instance_public_id_input,
        audit_action,
        actor_user_id,
        'indexer_instance_update'
    );

    RETURN indexer_instance_public_id_input;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_instance_update(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    display_name_input VARCHAR,
    priority_input INTEGER,
    trust_tier_key_input trust_tier_key,
    routing_policy_public_id_input UUID,
    is_enabled_input BOOLEAN,
    enable_rss_input BOOLEAN,
    enable_automatic_search_input BOOLEAN,
    enable_interactive_search_input BOOLEAN
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN indexer_instance_update_v1(
        actor_user_public_id,
        indexer_instance_public_id_input,
        display_name_input,
        priority_input,
        trust_tier_key_input,
        routing_policy_public_id_input,
        is_enabled_input,
        enable_rss_input,
        enable_automatic_search_input,
        enable_interactive_search_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_rss_subscription_set_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    is_enabled_input BOOLEAN,
    interval_seconds_input INTEGER
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to update RSS subscription';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    instance_is_enabled BOOLEAN;
    instance_enable_rss BOOLEAN;
    rss_row_exists BOOLEAN;
    rss_is_enabled BOOLEAN;
    rss_interval INTEGER;
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

    IF is_enabled_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'rss_enabled_missing';
    END IF;

    SELECT indexer_instance_id, deleted_at, is_enabled, enable_rss
    INTO instance_id, instance_deleted_at, instance_is_enabled, instance_enable_rss
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

    IF is_enabled_input AND (instance_is_enabled IS FALSE OR instance_enable_rss IS FALSE) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'rss_enable_indexer_disabled';
    END IF;

    IF interval_seconds_input IS NOT NULL THEN
        IF interval_seconds_input < 300 OR interval_seconds_input > 86400 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'interval_out_of_range';
        END IF;
    END IF;

    SELECT EXISTS (
        SELECT 1
        FROM indexer_rss_subscription
        WHERE indexer_instance_id = instance_id
    ) INTO rss_row_exists;

    IF rss_row_exists THEN
        SELECT is_enabled, interval_seconds
        INTO rss_is_enabled, rss_interval
        FROM indexer_rss_subscription
        WHERE indexer_instance_id = instance_id;

        IF is_enabled_input THEN
            IF rss_is_enabled THEN
                UPDATE indexer_rss_subscription
                SET interval_seconds = COALESCE(interval_seconds_input, interval_seconds)
                WHERE indexer_instance_id = instance_id;
            ELSE
                UPDATE indexer_rss_subscription
                SET is_enabled = TRUE,
                    interval_seconds = COALESCE(interval_seconds_input, interval_seconds),
                    last_error_class = NULL,
                    backoff_seconds = NULL,
                    next_poll_at = now() + make_interval(secs => random_jitter_seconds(60))
                WHERE indexer_instance_id = instance_id;
            END IF;
        ELSE
            UPDATE indexer_rss_subscription
            SET is_enabled = FALSE,
                interval_seconds = COALESCE(interval_seconds_input, interval_seconds),
                next_poll_at = NULL
            WHERE indexer_instance_id = instance_id;
        END IF;
    ELSE
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
            instance_id,
            is_enabled_input,
            COALESCE(interval_seconds_input, 900),
            NULL,
            CASE
                WHEN is_enabled_input THEN now() + make_interval(secs => random_jitter_seconds(60))
                ELSE NULL
            END,
            NULL,
            NULL
        );
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
        instance_id,
        indexer_instance_public_id_input,
        'update',
        actor_user_id,
        'rss_subscription_set'
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_rss_subscription_set(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    is_enabled_input BOOLEAN,
    interval_seconds_input INTEGER
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_rss_subscription_set_v1(
        actor_user_public_id,
        indexer_instance_public_id_input,
        is_enabled_input,
        interval_seconds_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_rss_subscription_disable_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to disable RSS subscription';
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

    SELECT indexer_instance_id, deleted_at
    INTO instance_id, instance_deleted_at
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
        instance_id,
        FALSE,
        900,
        NULL,
        NULL,
        NULL,
        NULL
    )
    ON CONFLICT (indexer_instance_id)
    DO UPDATE SET
        is_enabled = FALSE,
        next_poll_at = NULL;

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
        instance_id,
        indexer_instance_public_id_input,
        'update',
        actor_user_id,
        'rss_subscription_disable'
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_rss_subscription_disable(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_rss_subscription_disable_v1(
        actor_user_public_id,
        indexer_instance_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_instance_set_media_domains_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    media_domain_keys_input TEXT[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to set media domains';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    normalized_keys TEXT[];
    input_count INTEGER;
    resolved_count INTEGER;
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
    INTO instance_id, instance_deleted_at
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

    IF media_domain_keys_input IS NULL THEN
        normalized_keys := ARRAY[]::TEXT[];
    ELSE
        SELECT array_agg(DISTINCT lower(trim(value)))
        INTO normalized_keys
        FROM unnest(media_domain_keys_input) AS value;

        IF EXISTS (
            SELECT 1
            FROM unnest(media_domain_keys_input) AS value
            WHERE trim(value) = '' OR trim(value) <> lower(trim(value))
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'media_domain_key_invalid';
        END IF;
    END IF;

    IF normalized_keys IS NULL THEN
        normalized_keys := ARRAY[]::TEXT[];
    END IF;

    SELECT count(*)
    INTO input_count
    FROM unnest(normalized_keys) AS value
    WHERE value IS NOT NULL AND value <> '';

    IF input_count = 0 THEN
        DELETE FROM indexer_instance_media_domain
        WHERE indexer_instance_id = instance_id;
    ELSE
        SELECT count(*)
        INTO resolved_count
        FROM media_domain
        WHERE media_domain_key::TEXT = ANY(normalized_keys);

        IF resolved_count <> input_count THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'media_domain_not_found';
        END IF;

        DELETE FROM indexer_instance_media_domain
        WHERE indexer_instance_id = instance_id;

        INSERT INTO indexer_instance_media_domain (
            indexer_instance_id,
            media_domain_id
        )
        SELECT
            instance_id,
            media_domain_id
        FROM media_domain
        WHERE media_domain_key::TEXT = ANY(normalized_keys);
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
        instance_id,
        indexer_instance_public_id_input,
        'update',
        actor_user_id,
        'indexer_media_domains_set'
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_instance_set_media_domains(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    media_domain_keys_input TEXT[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_instance_set_media_domains_v1(
        actor_user_public_id,
        indexer_instance_public_id_input,
        media_domain_keys_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_instance_set_tags_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    tag_public_ids_input UUID[],
    tag_keys_input TEXT[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to set tags';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    resolved_ids_from_public UUID[];
    resolved_ids_from_keys UUID[];
    normalized_keys TEXT[];
    public_count INTEGER;
    public_resolved INTEGER;
    key_count INTEGER;
    key_resolved INTEGER;
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
    INTO instance_id, instance_deleted_at
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

    IF tag_public_ids_input IS NULL AND tag_keys_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tag_reference_missing';
    END IF;

    IF tag_public_ids_input IS NOT NULL THEN
        SELECT array_agg(DISTINCT tag_public_id), count(DISTINCT tag_public_id)
        INTO resolved_ids_from_public, public_resolved
        FROM tag
        WHERE tag_public_id = ANY(tag_public_ids_input)
          AND deleted_at IS NULL;

        SELECT count(DISTINCT value)
        INTO public_count
        FROM unnest(tag_public_ids_input) AS value;

        IF public_resolved IS NULL OR public_resolved <> public_count THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_not_found';
        END IF;
    END IF;

    IF tag_keys_input IS NOT NULL THEN
        SELECT array_agg(DISTINCT lower(trim(value)))
        INTO normalized_keys
        FROM unnest(tag_keys_input) AS value;

        IF EXISTS (
            SELECT 1
            FROM unnest(tag_keys_input) AS value
            WHERE trim(value) = ''
               OR trim(value) <> lower(trim(value))
               OR char_length(trim(value)) > 128
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_key_invalid';
        END IF;

        SELECT count(*)
        INTO key_count
        FROM unnest(normalized_keys) AS value
        WHERE value IS NOT NULL AND value <> '';

        IF key_count = 0 THEN
            normalized_keys := ARRAY[]::TEXT[];
        END IF;

        SELECT array_agg(DISTINCT tag_public_id), count(DISTINCT tag_public_id)
        INTO resolved_ids_from_keys, key_resolved
        FROM tag
        WHERE tag_key = ANY(normalized_keys)
          AND deleted_at IS NULL;

        IF key_resolved IS NULL OR key_resolved <> key_count THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_not_found';
        END IF;
    END IF;

    IF tag_public_ids_input IS NOT NULL AND tag_keys_input IS NOT NULL THEN
        IF EXISTS (
            SELECT value
            FROM unnest(resolved_ids_from_public) AS value
            EXCEPT
            SELECT value
            FROM unnest(resolved_ids_from_keys) AS value
        ) OR EXISTS (
            SELECT value
            FROM unnest(resolved_ids_from_keys) AS value
            EXCEPT
            SELECT value
            FROM unnest(resolved_ids_from_public) AS value
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_tag_reference';
        END IF;
    END IF;

    DELETE FROM indexer_instance_tag
    WHERE indexer_instance_id = instance_id;

    INSERT INTO indexer_instance_tag (
        indexer_instance_id,
        tag_id
    )
    SELECT
        instance_id,
        tag_id
    FROM tag
    WHERE tag_public_id = ANY(
        COALESCE(resolved_ids_from_public, resolved_ids_from_keys, ARRAY[]::UUID[])
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
        instance_id,
        indexer_instance_public_id_input,
        'update',
        actor_user_id,
        'indexer_tags_set'
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_instance_set_tags(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    tag_public_ids_input UUID[],
    tag_keys_input TEXT[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_instance_set_tags_v1(
        actor_user_public_id,
        indexer_instance_public_id_input,
        tag_public_ids_input,
        tag_keys_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_instance_field_set_value_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    field_name_input VARCHAR,
    value_plain_input VARCHAR,
    value_int_input INTEGER,
    value_decimal_input NUMERIC,
    value_bool_input BOOLEAN
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to set indexer field value';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    definition_id BIGINT;
    field_id BIGINT;
    field_type_value field_type;
    existing_field_type field_type;
    trimmed_field_name VARCHAR(128);
    trimmed_value_plain VARCHAR(2048);
    value_count INTEGER;
    validation_record RECORD;
    dep_value_plain VARCHAR(2048);
    dep_value_int INTEGER;
    dep_value_decimal NUMERIC;
    dep_value_bool BOOLEAN;
    condition_met BOOLEAN;
    value_set_type_value value_set_type;
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

    SELECT indexer_instance_id, deleted_at, indexer_definition_id
    INTO instance_id, instance_deleted_at, definition_id
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

    IF field_name_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'field_name_missing';
    END IF;

    trimmed_field_name := lower(trim(field_name_input));

    IF trimmed_field_name = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'field_name_empty';
    END IF;

    IF char_length(trimmed_field_name) > 128 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'field_name_too_long';
    END IF;

    SELECT indexer_definition_field_id, field_type
    INTO field_id, field_type_value
    FROM indexer_definition_field
    WHERE indexer_definition_id = definition_id
      AND name = trimmed_field_name;

    IF field_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'field_not_found';
    END IF;

    IF field_type_value IN ('password', 'api_key', 'cookie', 'token', 'header_value') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'field_requires_secret';
    END IF;

    trimmed_value_plain := NULL;
    IF value_plain_input IS NOT NULL THEN
        trimmed_value_plain := trim(value_plain_input);
        IF trimmed_value_plain = '' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'value_empty';
        END IF;
    END IF;

    value_count := (trimmed_value_plain IS NOT NULL)::INT
        + (value_int_input IS NOT NULL)::INT
        + (value_decimal_input IS NOT NULL)::INT
        + (value_bool_input IS NOT NULL)::INT;

    IF value_count <> 1 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'value_count_invalid';
    END IF;

    IF field_type_value IN ('string', 'select_single') THEN
        IF trimmed_value_plain IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'value_type_mismatch';
        END IF;
    ELSIF field_type_value = 'number_int' THEN
        IF value_int_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'value_type_mismatch';
        END IF;
    ELSIF field_type_value = 'number_decimal' THEN
        IF value_decimal_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'value_type_mismatch';
        END IF;
    ELSIF field_type_value = 'bool' THEN
        IF value_bool_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'value_type_mismatch';
        END IF;
    END IF;

    IF trimmed_value_plain IS NOT NULL AND char_length(trimmed_value_plain) > 2048 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'value_too_long';
    END IF;

    FOR validation_record IN
        SELECT
            validation_type,
            int_value,
            numeric_value,
            text_value,
            text_value_norm,
            value_set_id,
            depends_on_field_name,
            depends_on_operator,
            depends_on_value_plain_norm,
            depends_on_value_int,
            depends_on_value_bool,
            depends_on_value_set_id
        FROM indexer_definition_field_validation
        WHERE indexer_definition_field_id = field_id
    LOOP
        IF validation_record.validation_type = 'min_length' THEN
            IF trimmed_value_plain IS NOT NULL
                AND char_length(trimmed_value_plain) < validation_record.int_value THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'value_too_short';
            END IF;
        ELSIF validation_record.validation_type = 'max_length' THEN
            IF trimmed_value_plain IS NOT NULL
                AND char_length(trimmed_value_plain) > validation_record.int_value THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'value_too_long';
            END IF;
        ELSIF validation_record.validation_type = 'min_value' THEN
            IF value_int_input IS NOT NULL
                AND value_int_input < validation_record.numeric_value THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'value_too_small';
            END IF;
            IF value_decimal_input IS NOT NULL
                AND value_decimal_input < validation_record.numeric_value THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'value_too_small';
            END IF;
        ELSIF validation_record.validation_type = 'max_value' THEN
            IF value_int_input IS NOT NULL
                AND value_int_input > validation_record.numeric_value THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'value_too_large';
            END IF;
            IF value_decimal_input IS NOT NULL
                AND value_decimal_input > validation_record.numeric_value THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'value_too_large';
            END IF;
        ELSIF validation_record.validation_type = 'regex' THEN
            IF trimmed_value_plain IS NOT NULL
                AND trimmed_value_plain !~ validation_record.text_value THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'value_regex_mismatch';
            END IF;
        ELSIF validation_record.validation_type = 'allowed_value' THEN
            IF validation_record.value_set_id IS NULL THEN
                IF trimmed_value_plain IS NULL
                    OR lower(trimmed_value_plain) <> validation_record.text_value_norm THEN
                    RAISE EXCEPTION USING
                        ERRCODE = errcode,
                        MESSAGE = base_message,
                        DETAIL = 'value_not_allowed';
                END IF;
            ELSE
                SELECT value_set_type
                INTO value_set_type_value
                FROM indexer_definition_field_value_set
                WHERE value_set_id = validation_record.value_set_id;

                IF value_set_type_value = 'text' THEN
                    IF trimmed_value_plain IS NULL OR NOT EXISTS (
                        SELECT 1
                        FROM indexer_definition_field_value_set_item
                        WHERE value_set_id = validation_record.value_set_id
                          AND value_text = lower(trimmed_value_plain)
                    ) THEN
                        RAISE EXCEPTION USING
                            ERRCODE = errcode,
                            MESSAGE = base_message,
                            DETAIL = 'value_not_allowed';
                    END IF;
                ELSIF value_set_type_value = 'int' THEN
                    IF value_int_input IS NULL OR NOT EXISTS (
                        SELECT 1
                        FROM indexer_definition_field_value_set_item
                        WHERE value_set_id = validation_record.value_set_id
                          AND value_int = value_int_input
                    ) THEN
                        RAISE EXCEPTION USING
                            ERRCODE = errcode,
                            MESSAGE = base_message,
                            DETAIL = 'value_not_allowed';
                    END IF;
                ELSIF value_set_type_value = 'bigint' THEN
                    IF value_int_input IS NULL OR NOT EXISTS (
                        SELECT 1
                        FROM indexer_definition_field_value_set_item
                        WHERE value_set_id = validation_record.value_set_id
                          AND value_bigint = value_int_input::BIGINT
                    ) THEN
                        RAISE EXCEPTION USING
                            ERRCODE = errcode,
                            MESSAGE = base_message,
                            DETAIL = 'value_not_allowed';
                    END IF;
                END IF;
            END IF;
        ELSIF validation_record.validation_type = 'required_if_field_equals' THEN
            condition_met := FALSE;

            SELECT value_plain, value_int, value_decimal, value_bool
            INTO dep_value_plain, dep_value_int, dep_value_decimal, dep_value_bool
            FROM indexer_instance_field_value
            WHERE indexer_instance_id = instance_id
              AND field_name = validation_record.depends_on_field_name;

            IF validation_record.depends_on_operator = 'eq' THEN
                IF validation_record.depends_on_value_plain_norm IS NOT NULL THEN
                    condition_met := dep_value_plain IS NOT NULL
                        AND lower(trim(dep_value_plain)) = validation_record.depends_on_value_plain_norm;
                ELSIF validation_record.depends_on_value_int IS NOT NULL THEN
                    condition_met := dep_value_int IS NOT NULL
                        AND dep_value_int = validation_record.depends_on_value_int;
                ELSIF validation_record.depends_on_value_bool IS NOT NULL THEN
                    condition_met := dep_value_bool IS NOT NULL
                        AND dep_value_bool = validation_record.depends_on_value_bool;
                ELSIF validation_record.depends_on_value_set_id IS NOT NULL THEN
                    SELECT value_set_type
                    INTO value_set_type_value
                    FROM indexer_definition_field_value_set
                    WHERE value_set_id = validation_record.depends_on_value_set_id;

                    IF value_set_type_value = 'text' THEN
                        condition_met := dep_value_plain IS NOT NULL AND EXISTS (
                            SELECT 1
                            FROM indexer_definition_field_value_set_item
                            WHERE value_set_id = validation_record.depends_on_value_set_id
                              AND value_text = lower(trim(dep_value_plain))
                        );
                    ELSIF value_set_type_value = 'int' THEN
                        condition_met := dep_value_int IS NOT NULL AND EXISTS (
                            SELECT 1
                            FROM indexer_definition_field_value_set_item
                            WHERE value_set_id = validation_record.depends_on_value_set_id
                              AND value_int = dep_value_int
                        );
                    ELSIF value_set_type_value = 'bigint' THEN
                        condition_met := dep_value_int IS NOT NULL AND EXISTS (
                            SELECT 1
                            FROM indexer_definition_field_value_set_item
                            WHERE value_set_id = validation_record.depends_on_value_set_id
                              AND value_bigint = dep_value_int::BIGINT
                        );
                    END IF;
                END IF;
            ELSIF validation_record.depends_on_operator = 'neq' THEN
                IF validation_record.depends_on_value_plain_norm IS NOT NULL THEN
                    condition_met := dep_value_plain IS NOT NULL
                        AND lower(trim(dep_value_plain)) <> validation_record.depends_on_value_plain_norm;
                ELSIF validation_record.depends_on_value_int IS NOT NULL THEN
                    condition_met := dep_value_int IS NOT NULL
                        AND dep_value_int <> validation_record.depends_on_value_int;
                ELSIF validation_record.depends_on_value_bool IS NOT NULL THEN
                    condition_met := dep_value_bool IS NOT NULL
                        AND dep_value_bool <> validation_record.depends_on_value_bool;
                ELSIF validation_record.depends_on_value_set_id IS NOT NULL THEN
                    SELECT value_set_type
                    INTO value_set_type_value
                    FROM indexer_definition_field_value_set
                    WHERE value_set_id = validation_record.depends_on_value_set_id;

                    IF value_set_type_value = 'text' THEN
                        condition_met := dep_value_plain IS NOT NULL AND NOT EXISTS (
                            SELECT 1
                            FROM indexer_definition_field_value_set_item
                            WHERE value_set_id = validation_record.depends_on_value_set_id
                              AND value_text = lower(trim(dep_value_plain))
                        );
                    ELSIF value_set_type_value = 'int' THEN
                        condition_met := dep_value_int IS NOT NULL AND NOT EXISTS (
                            SELECT 1
                            FROM indexer_definition_field_value_set_item
                            WHERE value_set_id = validation_record.depends_on_value_set_id
                              AND value_int = dep_value_int
                        );
                    ELSIF value_set_type_value = 'bigint' THEN
                        condition_met := dep_value_int IS NOT NULL AND NOT EXISTS (
                            SELECT 1
                            FROM indexer_definition_field_value_set_item
                            WHERE value_set_id = validation_record.depends_on_value_set_id
                              AND value_bigint = dep_value_int::BIGINT
                        );
                    END IF;
                END IF;
            ELSIF validation_record.depends_on_operator = 'in_set' THEN
                IF validation_record.depends_on_value_set_id IS NOT NULL THEN
                    SELECT value_set_type
                    INTO value_set_type_value
                    FROM indexer_definition_field_value_set
                    WHERE value_set_id = validation_record.depends_on_value_set_id;

                    IF value_set_type_value = 'text' THEN
                        condition_met := dep_value_plain IS NOT NULL AND EXISTS (
                            SELECT 1
                            FROM indexer_definition_field_value_set_item
                            WHERE value_set_id = validation_record.depends_on_value_set_id
                              AND value_text = lower(trim(dep_value_plain))
                        );
                    ELSIF value_set_type_value = 'int' THEN
                        condition_met := dep_value_int IS NOT NULL AND EXISTS (
                            SELECT 1
                            FROM indexer_definition_field_value_set_item
                            WHERE value_set_id = validation_record.depends_on_value_set_id
                              AND value_int = dep_value_int
                        );
                    ELSIF value_set_type_value = 'bigint' THEN
                        condition_met := dep_value_int IS NOT NULL AND EXISTS (
                            SELECT 1
                            FROM indexer_definition_field_value_set_item
                            WHERE value_set_id = validation_record.depends_on_value_set_id
                              AND value_bigint = dep_value_int::BIGINT
                        );
                    END IF;
                END IF;
            END IF;

            IF condition_met AND value_count = 0 THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'value_required';
            END IF;
        END IF;
    END LOOP;

    SELECT field_type
    INTO existing_field_type
    FROM indexer_instance_field_value
    WHERE indexer_instance_id = instance_id
      AND field_name = trimmed_field_name;

    IF existing_field_type IS NOT NULL AND existing_field_type <> field_type_value THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'field_type_mismatch';
    END IF;

    INSERT INTO indexer_instance_field_value (
        indexer_instance_id,
        field_name,
        field_type,
        value_plain,
        value_int,
        value_decimal,
        value_bool,
        updated_by_user_id
    )
    VALUES (
        instance_id,
        trimmed_field_name,
        field_type_value,
        trimmed_value_plain,
        value_int_input,
        value_decimal_input,
        value_bool_input,
        actor_user_id
    )
    ON CONFLICT (indexer_instance_id, field_name)
    DO UPDATE SET
        field_type = EXCLUDED.field_type,
        value_plain = EXCLUDED.value_plain,
        value_int = EXCLUDED.value_int,
        value_decimal = EXCLUDED.value_decimal,
        value_bool = EXCLUDED.value_bool,
        updated_by_user_id = EXCLUDED.updated_by_user_id,
        updated_at = now();

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'indexer_instance_field_value',
        (SELECT indexer_instance_field_value_id
         FROM indexer_instance_field_value
         WHERE indexer_instance_id = instance_id
           AND field_name = trimmed_field_name),
        NULL,
        'update',
        actor_user_id,
        'indexer_field_set'
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_instance_field_set_value(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    field_name_input VARCHAR,
    value_plain_input VARCHAR,
    value_int_input INTEGER,
    value_decimal_input NUMERIC,
    value_bool_input BOOLEAN
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_instance_field_set_value_v1(
        actor_user_public_id,
        indexer_instance_public_id_input,
        field_name_input,
        value_plain_input,
        value_int_input,
        value_decimal_input,
        value_bool_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_instance_field_bind_secret_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    field_name_input VARCHAR,
    secret_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to bind indexer field secret';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    definition_id BIGINT;
    field_id BIGINT;
    field_type_value field_type;
    trimmed_field_name VARCHAR(128);
    secret_id_value BIGINT;
    binding_name_value secret_binding_name;
    field_value_id BIGINT;
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

    IF secret_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_missing';
    END IF;

    SELECT indexer_instance_id, deleted_at, indexer_definition_id
    INTO instance_id, instance_deleted_at, definition_id
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

    IF field_name_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'field_name_missing';
    END IF;

    trimmed_field_name := lower(trim(field_name_input));

    IF trimmed_field_name = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'field_name_empty';
    END IF;

    IF char_length(trimmed_field_name) > 128 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'field_name_too_long';
    END IF;

    SELECT indexer_definition_field_id, field_type
    INTO field_id, field_type_value
    FROM indexer_definition_field
    WHERE indexer_definition_id = definition_id
      AND name = trimmed_field_name;

    IF field_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'field_not_found';
    END IF;

    IF field_type_value NOT IN ('password', 'api_key', 'cookie', 'token', 'header_value') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'field_not_secret';
    END IF;

    SELECT secret_id
    INTO secret_id_value
    FROM secret
    WHERE secret_public_id = secret_public_id_input
      AND is_revoked = FALSE;

    IF secret_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_not_found';
    END IF;

    binding_name_value := CASE field_type_value
        WHEN 'password' THEN 'password'
        WHEN 'api_key' THEN 'api_key'
        WHEN 'cookie' THEN 'cookie'
        WHEN 'token' THEN 'token'
        WHEN 'header_value' THEN 'header_value'
        ELSE 'password'
    END;

    INSERT INTO indexer_instance_field_value (
        indexer_instance_id,
        field_name,
        field_type,
        value_plain,
        value_int,
        value_decimal,
        value_bool,
        updated_by_user_id
    )
    VALUES (
        instance_id,
        trimmed_field_name,
        field_type_value,
        NULL,
        NULL,
        NULL,
        NULL,
        actor_user_id
    )
    ON CONFLICT (indexer_instance_id, field_name)
    DO UPDATE SET
        field_type = EXCLUDED.field_type,
        value_plain = NULL,
        value_int = NULL,
        value_decimal = NULL,
        value_bool = NULL,
        updated_by_user_id = EXCLUDED.updated_by_user_id,
        updated_at = now();

    SELECT indexer_instance_field_value_id
    INTO field_value_id
    FROM indexer_instance_field_value
    WHERE indexer_instance_id = instance_id
      AND field_name = trimmed_field_name;

    DELETE FROM secret_binding
    WHERE bound_table = 'indexer_instance_field_value'
      AND bound_id = field_value_id
      AND binding_name = binding_name_value;

    INSERT INTO secret_binding (
        secret_id,
        bound_table,
        bound_id,
        binding_name
    )
    VALUES (
        secret_id_value,
        'indexer_instance_field_value',
        field_value_id,
        binding_name_value
    );

    INSERT INTO secret_audit_log (
        secret_id,
        action,
        actor_user_id,
        detail
    )
    VALUES (
        secret_id_value,
        'bind',
        actor_user_id,
        'indexer_field_bind'
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
        'indexer_instance_field_value',
        field_value_id,
        NULL,
        'update',
        actor_user_id,
        'indexer_field_bind_secret'
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_instance_field_bind_secret(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    field_name_input VARCHAR,
    secret_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_instance_field_bind_secret_v1(
        actor_user_public_id,
        indexer_instance_public_id_input,
        field_name_input,
        secret_public_id_input
    );
END;
$$;
