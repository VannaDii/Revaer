-- Search request cancel procedure.

CREATE OR REPLACE FUNCTION search_request_cancel_v1(
    actor_user_public_id UUID,
    search_request_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to cancel search request';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    request_id BIGINT;
    request_user_id BIGINT;
    request_status search_status;
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

    IF search_request_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_missing';
    END IF;

    SELECT search_request_id, user_id, status
    INTO request_id, request_user_id, request_status
    FROM search_request
    WHERE search_request_public_id = search_request_public_id_input;

    IF request_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_not_found';
    END IF;

    IF request_user_id IS NULL THEN
        IF actor_role NOT IN ('owner', 'admin') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_unauthorized';
        END IF;
    ELSE
        IF request_user_id <> actor_user_id AND actor_role NOT IN ('owner', 'admin') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_unauthorized';
        END IF;
    END IF;

    IF request_status IN ('canceled', 'finished', 'failed') THEN
        RETURN;
    END IF;

    UPDATE search_request
    SET status = 'canceled',
        canceled_at = now(),
        finished_at = now(),
        failure_class = NULL,
        error_detail = NULL
    WHERE search_request_id = request_id;

    UPDATE search_request_indexer_run
    SET status = 'canceled',
        started_at = COALESCE(started_at, now()),
        finished_at = now(),
        next_attempt_at = NULL,
        error_class = NULL,
        error_detail = NULL
    WHERE search_request_id = request_id
      AND status IN ('queued', 'running');
END;
$$;

CREATE OR REPLACE FUNCTION search_request_cancel(
    actor_user_public_id UUID,
    search_request_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_request_cancel_v1(
        actor_user_public_id,
        search_request_public_id_input
    );
END;
$$;
