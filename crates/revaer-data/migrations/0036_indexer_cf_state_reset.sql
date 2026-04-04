-- Stored procedure for Cloudflare state reset.

CREATE OR REPLACE FUNCTION indexer_cf_state_reset_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    reason_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    errcode CONSTANT text := 'P0001';
    message CONSTANT text := 'Failed to reset Cloudflare state';
    actor_user_id BIGINT;
    actor_role deployment_role;
    indexer_id BIGINT;
    indexer_deleted_at TIMESTAMPTZ;
    connectivity_status_value connectivity_status;
    connectivity_error_class error_class;
    trimmed_reason VARCHAR(1024);
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = message,
            DETAIL = 'actor_missing';
    END IF;

    SELECT user_id, role
    INTO actor_user_id, actor_role
    FROM app_user
    WHERE user_public_id = actor_user_public_id;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = message,
            DETAIL = 'actor_not_found';
    END IF;

    IF actor_role NOT IN ('owner', 'admin') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = message,
            DETAIL = 'actor_unauthorized';
    END IF;

    IF indexer_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = message,
            DETAIL = 'indexer_missing';
    END IF;

    IF reason_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = message,
            DETAIL = 'reason_missing';
    END IF;

    trimmed_reason := trim(reason_input);

    IF trimmed_reason = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = message,
            DETAIL = 'reason_empty';
    END IF;

    IF char_length(trimmed_reason) > 1024 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = message,
            DETAIL = 'reason_too_long';
    END IF;

    SELECT indexer_instance_id, deleted_at
    INTO indexer_id, indexer_deleted_at
    FROM indexer_instance
    WHERE indexer_instance_public_id = indexer_instance_public_id_input;

    IF indexer_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = message,
            DETAIL = 'indexer_not_found';
    END IF;

    IF indexer_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = message,
            DETAIL = 'indexer_deleted';
    END IF;

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
        indexer_id,
        'clear',
        now(),
        NULL,
        NULL,
        NULL,
        NULL,
        0,
        NULL
    )
    ON CONFLICT (indexer_instance_id)
    DO UPDATE SET
        state = 'clear',
        last_changed_at = now(),
        cf_session_id = NULL,
        cf_session_expires_at = NULL,
        cooldown_until = NULL,
        backoff_seconds = NULL,
        consecutive_failures = 0,
        last_error_class = NULL;

    SELECT status, error_class
    INTO connectivity_status_value, connectivity_error_class
    FROM indexer_connectivity_profile
    WHERE indexer_instance_id = indexer_id;

    IF connectivity_status_value = 'quarantined'
        AND connectivity_error_class IN ('cf_challenge', 'http_429') THEN
        UPDATE indexer_connectivity_profile
        SET status = 'degraded',
            error_class = 'unknown',
            last_checked_at = now()
        WHERE indexer_instance_id = indexer_id;
    END IF;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary,
        change_detail
    )
    VALUES (
        'indexer_instance',
        indexer_id,
        indexer_instance_public_id_input,
        'update',
        actor_user_id,
        'cf_state_reset',
        trimmed_reason
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_cf_state_reset(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    reason_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_cf_state_reset_v1(
        actor_user_public_id,
        indexer_instance_public_id_input,
        reason_input
    );
END;
$$;
