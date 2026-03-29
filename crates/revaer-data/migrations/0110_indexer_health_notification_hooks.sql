-- Operator-managed notification hooks for indexer health alerts.

CREATE TYPE indexer_health_notification_channel AS ENUM (
    'email',
    'webhook'
);

CREATE TYPE indexer_health_notification_threshold AS ENUM (
    'degraded',
    'failing',
    'quarantined'
);

CREATE TABLE IF NOT EXISTS indexer_health_notification_hook (
    indexer_health_notification_hook_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_health_notification_hook_public_id UUID NOT NULL DEFAULT gen_random_uuid(),
    channel indexer_health_notification_channel NOT NULL,
    display_name VARCHAR(120) NOT NULL,
    status_threshold indexer_health_notification_threshold NOT NULL DEFAULT 'failing',
    webhook_url VARCHAR(2048),
    email VARCHAR(320),
    email_normalized VARCHAR(320),
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_user_id BIGINT NOT NULL REFERENCES app_user (user_id),
    updated_by_user_id BIGINT NOT NULL REFERENCES app_user (user_id),
    CONSTRAINT indexer_health_notification_hook_public_id_uq UNIQUE (
        indexer_health_notification_hook_public_id
    ),
    CONSTRAINT indexer_health_notification_hook_email_normalized_lc CHECK (
        email_normalized IS NULL OR email_normalized = lower(trim(email_normalized))
    ),
    CONSTRAINT indexer_health_notification_hook_channel_payload_ck CHECK (
        (
            channel = 'webhook'
            AND webhook_url IS NOT NULL
            AND email IS NULL
            AND email_normalized IS NULL
        )
        OR
        (
            channel = 'email'
            AND webhook_url IS NULL
            AND email IS NOT NULL
            AND email_normalized IS NOT NULL
        )
    )
);

CREATE INDEX indexer_health_notification_hook_enabled_idx
    ON indexer_health_notification_hook (is_enabled, status_threshold, channel);

CREATE OR REPLACE FUNCTION indexer_health_notification_hook_create_v1(
    actor_user_public_id UUID,
    channel_input indexer_health_notification_channel,
    display_name_input VARCHAR,
    status_threshold_input indexer_health_notification_threshold,
    webhook_url_input VARCHAR,
    email_input VARCHAR
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to create health notification hook';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    normalized_display_name VARCHAR(120);
    normalized_email VARCHAR(320);
    normalized_webhook_url VARCHAR(2048);
    hook_public_id UUID;
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

    normalized_display_name := NULLIF(trim(display_name_input), '');
    IF normalized_display_name IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'display_name_missing';
    END IF;

    IF status_threshold_input IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'status_threshold_missing';
    END IF;

    CASE channel_input
        WHEN 'webhook' THEN
            normalized_webhook_url := NULLIF(trim(webhook_url_input), '');
            IF normalized_webhook_url IS NULL THEN
                RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'webhook_url_missing';
            END IF;
            IF normalized_webhook_url !~ '^https?://.+' THEN
                RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'webhook_url_invalid';
            END IF;
        WHEN 'email' THEN
            normalized_email := NULLIF(trim(email_input), '');
            IF normalized_email IS NULL THEN
                RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'email_missing';
            END IF;
            normalized_email := lower(normalized_email);
            IF position('@' IN normalized_email) < 2 THEN
                RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'email_invalid';
            END IF;
        ELSE
            RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'channel_invalid';
    END CASE;

    INSERT INTO indexer_health_notification_hook (
        channel,
        display_name,
        status_threshold,
        webhook_url,
        email,
        email_normalized,
        created_by_user_id,
        updated_by_user_id
    )
    VALUES (
        channel_input,
        normalized_display_name,
        status_threshold_input,
        normalized_webhook_url,
        normalized_email,
        normalized_email,
        actor_user_id,
        actor_user_id
    )
    RETURNING indexer_health_notification_hook_public_id
    INTO hook_public_id;

    RETURN hook_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_health_notification_hook_create(
    actor_user_public_id UUID,
    channel_input indexer_health_notification_channel,
    display_name_input VARCHAR,
    status_threshold_input indexer_health_notification_threshold,
    webhook_url_input VARCHAR,
    email_input VARCHAR
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN indexer_health_notification_hook_create_v1(
        actor_user_public_id,
        channel_input,
        display_name_input,
        status_threshold_input,
        webhook_url_input,
        email_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_health_notification_hook_update_v1(
    actor_user_public_id UUID,
    indexer_health_notification_hook_public_id_input UUID,
    display_name_input VARCHAR,
    status_threshold_input indexer_health_notification_threshold,
    webhook_url_input VARCHAR,
    email_input VARCHAR,
    is_enabled_input BOOLEAN
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to update health notification hook';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    hook_id BIGINT;
    current_channel indexer_health_notification_channel;
    next_display_name VARCHAR(120);
    next_status_threshold indexer_health_notification_threshold;
    next_webhook_url VARCHAR(2048);
    next_email VARCHAR(320);
    next_email_normalized VARCHAR(320);
    next_is_enabled BOOLEAN;
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

    SELECT
        indexer_health_notification_hook_id,
        channel,
        display_name,
        status_threshold,
        webhook_url,
        email,
        email_normalized,
        is_enabled
    INTO
        hook_id,
        current_channel,
        next_display_name,
        next_status_threshold,
        next_webhook_url,
        next_email,
        next_email_normalized,
        next_is_enabled
    FROM indexer_health_notification_hook
    WHERE indexer_health_notification_hook_public_id = indexer_health_notification_hook_public_id_input;

    IF hook_id IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'hook_not_found';
    END IF;

    IF display_name_input IS NOT NULL THEN
        next_display_name := NULLIF(trim(display_name_input), '');
        IF next_display_name IS NULL THEN
            RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'display_name_missing';
        END IF;
    END IF;

    next_status_threshold := COALESCE(status_threshold_input, next_status_threshold);
    next_is_enabled := COALESCE(is_enabled_input, next_is_enabled);

    IF current_channel = 'webhook' THEN
        IF webhook_url_input IS NOT NULL THEN
            next_webhook_url := NULLIF(trim(webhook_url_input), '');
            IF next_webhook_url IS NULL THEN
                RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'webhook_url_missing';
            END IF;
            IF next_webhook_url !~ '^https?://.+' THEN
                RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'webhook_url_invalid';
            END IF;
        END IF;
        IF email_input IS NOT NULL THEN
            RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'channel_payload_mismatch';
        END IF;
        next_email := NULL;
        next_email_normalized := NULL;
    ELSE
        IF email_input IS NOT NULL THEN
            next_email := NULLIF(trim(email_input), '');
            IF next_email IS NULL THEN
                RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'email_missing';
            END IF;
            next_email_normalized := lower(next_email);
            IF position('@' IN next_email_normalized) < 2 THEN
                RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'email_invalid';
            END IF;
        END IF;
        IF webhook_url_input IS NOT NULL THEN
            RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'channel_payload_mismatch';
        END IF;
        next_webhook_url := NULL;
    END IF;

    UPDATE indexer_health_notification_hook
    SET
        display_name = next_display_name,
        status_threshold = next_status_threshold,
        webhook_url = next_webhook_url,
        email = next_email,
        email_normalized = next_email_normalized,
        is_enabled = next_is_enabled,
        updated_at = now(),
        updated_by_user_id = actor_user_id
    WHERE indexer_health_notification_hook_id = hook_id;

    RETURN indexer_health_notification_hook_public_id_input;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_health_notification_hook_update(
    actor_user_public_id UUID,
    indexer_health_notification_hook_public_id_input UUID,
    display_name_input VARCHAR,
    status_threshold_input indexer_health_notification_threshold,
    webhook_url_input VARCHAR,
    email_input VARCHAR,
    is_enabled_input BOOLEAN
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN indexer_health_notification_hook_update_v1(
        actor_user_public_id,
        indexer_health_notification_hook_public_id_input,
        display_name_input,
        status_threshold_input,
        webhook_url_input,
        email_input,
        is_enabled_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_health_notification_hook_delete_v1(
    actor_user_public_id UUID,
    indexer_health_notification_hook_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to delete health notification hook';
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

    DELETE FROM indexer_health_notification_hook
    WHERE indexer_health_notification_hook_public_id = indexer_health_notification_hook_public_id_input;

    IF NOT FOUND THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'hook_not_found';
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_health_notification_hook_delete(
    actor_user_public_id UUID,
    indexer_health_notification_hook_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_health_notification_hook_delete_v1(
        actor_user_public_id,
        indexer_health_notification_hook_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_health_notification_hook_list_v1(
    actor_user_public_id UUID
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
    base_message CONSTANT text := 'Failed to list health notification hooks';
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
    ORDER BY hook.display_name ASC, hook.indexer_health_notification_hook_id ASC;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_health_notification_hook_list(
    actor_user_public_id UUID
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
    FROM indexer_health_notification_hook_list_v1(actor_user_public_id);
END;
$$;
