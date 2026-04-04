-- Fetch Cloudflare state for a specific indexer instance.

CREATE OR REPLACE FUNCTION indexer_cf_state_get_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID
)
RETURNS TABLE (
    state cf_state,
    last_changed_at TIMESTAMPTZ,
    cf_session_expires_at TIMESTAMPTZ,
    cooldown_until TIMESTAMPTZ,
    backoff_seconds INTEGER,
    consecutive_failures INTEGER,
    last_error_class error_class
) AS
$body$
DECLARE
    errcode CONSTANT text := 'P0001';
    v_indexer_instance_id BIGINT;
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION 'actor_missing' USING ERRCODE = errcode, DETAIL = 'actor_missing';
    END IF;

    SELECT indexer_instance_id
    INTO v_indexer_instance_id
    FROM indexer_instance
    WHERE indexer_instance_public_id = indexer_instance_public_id_input
      AND deleted_at IS NULL;

    IF v_indexer_instance_id IS NULL THEN
        RAISE EXCEPTION 'indexer_not_found' USING ERRCODE = errcode, DETAIL = 'indexer_not_found';
    END IF;

    RETURN QUERY
    SELECT
        state,
        last_changed_at,
        cf_session_expires_at,
        cooldown_until,
        backoff_seconds,
        consecutive_failures,
        last_error_class
    FROM indexer_cf_state
    WHERE indexer_instance_id = v_indexer_instance_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'indexer_not_found' USING ERRCODE = errcode, DETAIL = 'indexer_not_found';
    END IF;
END;
$body$
LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION indexer_cf_state_get(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID
)
RETURNS TABLE (
    state cf_state,
    last_changed_at TIMESTAMPTZ,
    cf_session_expires_at TIMESTAMPTZ,
    cooldown_until TIMESTAMPTZ,
    backoff_seconds INTEGER,
    consecutive_failures INTEGER,
    last_error_class error_class
) AS
$wrapper$
    SELECT *
    FROM indexer_cf_state_get_v1(actor_user_public_id, indexer_instance_public_id_input);
$wrapper$
LANGUAGE SQL
STABLE;
