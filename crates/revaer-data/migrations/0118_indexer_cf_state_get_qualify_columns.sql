-- Qualify returned Cloudflare-state columns so PL/pgSQL output variables do not shadow table columns.

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
        cf.state,
        cf.last_changed_at,
        cf.cf_session_expires_at,
        cf.cooldown_until,
        cf.backoff_seconds,
        cf.consecutive_failures,
        cf.last_error_class
    FROM indexer_cf_state AS cf
    WHERE cf.indexer_instance_id = v_indexer_instance_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'indexer_not_found' USING ERRCODE = errcode, DETAIL = 'indexer_not_found';
    END IF;
END;
$body$
LANGUAGE plpgsql;
