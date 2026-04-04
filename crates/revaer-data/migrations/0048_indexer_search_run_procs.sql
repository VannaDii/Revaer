-- Search indexer run procedures.

CREATE OR REPLACE FUNCTION search_indexer_run_enqueue_v1(
    search_request_public_id_input UUID,
    indexer_instance_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to enqueue indexer run';
    errcode CONSTANT text := 'P0001';
    request_id BIGINT;
    request_status search_status;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    run_id BIGINT;
    run_status run_status;
BEGIN
    IF search_request_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_missing';
    END IF;

    IF indexer_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_missing';
    END IF;

    SELECT search_request_id, status
    INTO request_id, request_status
    FROM search_request
    WHERE search_request_public_id = search_request_public_id_input;

    IF request_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_not_found';
    END IF;

    IF request_status <> 'running' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_not_running';
    END IF;

    SELECT indexer_instance_id, deleted_at
    INTO instance_id, instance_deleted_at
    FROM indexer_instance
    WHERE indexer_instance_public_id = indexer_instance_public_id_input;

    IF instance_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_not_found';
    END IF;

    IF instance_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_deleted';
    END IF;

    SELECT search_request_indexer_run_id, status
    INTO run_id, run_status
    FROM search_request_indexer_run
    WHERE search_request_id = request_id
      AND indexer_instance_id = instance_id;

    IF run_id IS NOT NULL THEN
        IF run_status = 'queued' THEN
            RETURN;
        END IF;

        IF run_status = 'running' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'run_already_running';
        END IF;

        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'run_already_terminal';
    END IF;

    INSERT INTO search_request_indexer_run (
        search_request_id,
        indexer_instance_id,
        status,
        attempt_count,
        rate_limited_attempt_count,
        items_seen_count,
        items_emitted_count,
        canonical_added_count
    )
    VALUES (
        request_id,
        instance_id,
        'queued',
        0,
        0,
        0,
        0,
        0
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_indexer_run_enqueue(
    search_request_public_id_input UUID,
    indexer_instance_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_indexer_run_enqueue_v1(
        search_request_public_id_input,
        indexer_instance_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_indexer_run_mark_started_v1(
    search_request_public_id_input UUID,
    indexer_instance_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to mark indexer run started';
    errcode CONSTANT text := 'P0001';
    request_id BIGINT;
    request_status search_status;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    run_id BIGINT;
    run_status run_status;
BEGIN
    IF search_request_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_missing';
    END IF;

    IF indexer_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_missing';
    END IF;

    SELECT search_request_id, status
    INTO request_id, request_status
    FROM search_request
    WHERE search_request_public_id = search_request_public_id_input;

    IF request_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_not_found';
    END IF;

    IF request_status <> 'running' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_not_running';
    END IF;

    SELECT indexer_instance_id, deleted_at
    INTO instance_id, instance_deleted_at
    FROM indexer_instance
    WHERE indexer_instance_public_id = indexer_instance_public_id_input;

    IF instance_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_not_found';
    END IF;

    IF instance_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_deleted';
    END IF;

    SELECT search_request_indexer_run_id, status
    INTO run_id, run_status
    FROM search_request_indexer_run
    WHERE search_request_id = request_id
      AND indexer_instance_id = instance_id;

    IF run_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'run_not_found';
    END IF;

    IF run_status <> 'queued' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'run_invalid_state';
    END IF;

    UPDATE search_request_indexer_run
    SET status = 'running',
        started_at = now(),
        next_attempt_at = NULL,
        attempt_count = attempt_count + 1
    WHERE search_request_indexer_run_id = run_id;
END;
$$;

CREATE OR REPLACE FUNCTION search_indexer_run_mark_started(
    search_request_public_id_input UUID,
    indexer_instance_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_indexer_run_mark_started_v1(
        search_request_public_id_input,
        indexer_instance_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_indexer_run_mark_finished_v1(
    search_request_public_id_input UUID,
    indexer_instance_public_id_input UUID,
    items_seen_delta_input INTEGER,
    items_emitted_delta_input INTEGER,
    canonical_added_delta_input INTEGER
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to mark indexer run finished';
    errcode CONSTANT text := 'P0001';
    request_id BIGINT;
    request_status search_status;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    run_id BIGINT;
    run_status run_status;
    items_seen_delta INTEGER;
    items_emitted_delta INTEGER;
    canonical_added_delta INTEGER;
BEGIN
    IF search_request_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_missing';
    END IF;

    IF indexer_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_missing';
    END IF;

    SELECT search_request_id, status
    INTO request_id, request_status
    FROM search_request
    WHERE search_request_public_id = search_request_public_id_input;

    IF request_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_not_found';
    END IF;

    IF request_status <> 'running' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_not_running';
    END IF;

    SELECT indexer_instance_id, deleted_at
    INTO instance_id, instance_deleted_at
    FROM indexer_instance
    WHERE indexer_instance_public_id = indexer_instance_public_id_input;

    IF instance_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_not_found';
    END IF;

    IF instance_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_deleted';
    END IF;

    SELECT search_request_indexer_run_id, status
    INTO run_id, run_status
    FROM search_request_indexer_run
    WHERE search_request_id = request_id
      AND indexer_instance_id = instance_id;

    IF run_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'run_not_found';
    END IF;

    IF run_status <> 'running' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'run_invalid_state';
    END IF;

    items_seen_delta := COALESCE(items_seen_delta_input, 0);
    items_emitted_delta := COALESCE(items_emitted_delta_input, 0);
    canonical_added_delta := COALESCE(canonical_added_delta_input, 0);

    IF items_seen_delta < 0 OR items_emitted_delta < 0 OR canonical_added_delta < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'delta_negative';
    END IF;

    UPDATE search_request_indexer_run
    SET status = 'finished',
        started_at = COALESCE(started_at, now()),
        finished_at = now(),
        next_attempt_at = NULL,
        error_class = NULL,
        error_detail = NULL,
        items_seen_count = items_seen_count + items_seen_delta,
        items_emitted_count = items_emitted_count + items_emitted_delta,
        canonical_added_count = canonical_added_count + canonical_added_delta
    WHERE search_request_indexer_run_id = run_id;
END;
$$;

CREATE OR REPLACE FUNCTION search_indexer_run_mark_finished(
    search_request_public_id_input UUID,
    indexer_instance_public_id_input UUID,
    items_seen_delta_input INTEGER,
    items_emitted_delta_input INTEGER,
    canonical_added_delta_input INTEGER
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_indexer_run_mark_finished_v1(
        search_request_public_id_input,
        indexer_instance_public_id_input,
        items_seen_delta_input,
        items_emitted_delta_input,
        canonical_added_delta_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_indexer_run_mark_failed_v1(
    search_request_public_id_input UUID,
    indexer_instance_public_id_input UUID,
    error_class_input error_class,
    error_detail_input VARCHAR,
    retry_after_seconds_input INTEGER,
    retry_seq_input SMALLINT,
    rate_limit_scope_input rate_limit_scope
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to mark indexer run failed';
    errcode CONSTANT text := 'P0001';
    request_id BIGINT;
    request_status search_status;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    run_id BIGINT;
    run_status run_status;
    max_retry_seq SMALLINT;
    retry_seq_value SMALLINT;
    delay_no_jitter INTEGER;
    jitter_pct INTEGER;
    jitter_seconds INTEGER;
    next_attempt TIMESTAMPTZ;
    new_rate_limited_count INTEGER;
BEGIN
    IF search_request_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_missing';
    END IF;

    IF indexer_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_missing';
    END IF;

    IF error_class_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'error_class_missing';
    END IF;

    IF error_detail_input IS NOT NULL AND char_length(error_detail_input) > 1024 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'error_detail_too_long';
    END IF;

    SELECT search_request_id, status
    INTO request_id, request_status
    FROM search_request
    WHERE search_request_public_id = search_request_public_id_input;

    IF request_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_not_found';
    END IF;

    IF request_status <> 'running' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_not_running';
    END IF;

    SELECT indexer_instance_id, deleted_at
    INTO instance_id, instance_deleted_at
    FROM indexer_instance
    WHERE indexer_instance_public_id = indexer_instance_public_id_input;

    IF instance_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_not_found';
    END IF;

    IF instance_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_deleted';
    END IF;

    SELECT search_request_indexer_run_id, status
    INTO run_id, run_status
    FROM search_request_indexer_run
    WHERE search_request_id = request_id
      AND indexer_instance_id = instance_id;

    IF run_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'run_not_found';
    END IF;

    IF run_status IN ('finished', 'failed', 'canceled') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'run_already_terminal';
    END IF;

    IF error_class_input = 'rate_limited' THEN
        IF run_status <> 'queued' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'run_invalid_state';
        END IF;

        IF rate_limit_scope_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'rate_limit_scope_missing';
        END IF;

        UPDATE search_request_indexer_run
        SET attempt_count = attempt_count + 1,
            rate_limited_attempt_count = rate_limited_attempt_count + 1,
            last_error_class = 'rate_limited',
            last_rate_limit_scope = rate_limit_scope_input
        WHERE search_request_indexer_run_id = run_id
        RETURNING rate_limited_attempt_count
        INTO new_rate_limited_count;

        IF new_rate_limited_count >= 10 THEN
            UPDATE search_request_indexer_run
            SET status = 'failed',
                started_at = COALESCE(started_at, now()),
                finished_at = now(),
                next_attempt_at = NULL,
                error_class = 'rate_limited',
                error_detail = error_detail_input
            WHERE search_request_indexer_run_id = run_id;

            RETURN;
        END IF;

        delay_no_jitter := LEAST(5 * (1 << (new_rate_limited_count - 1)), 300);
        jitter_pct := random_jitter_seconds(25);
        jitter_seconds := (delay_no_jitter * jitter_pct) / 100;
        next_attempt := now() + make_interval(secs => delay_no_jitter + jitter_seconds);

        UPDATE search_request_indexer_run
        SET status = 'queued',
            started_at = NULL,
            next_attempt_at = next_attempt
        WHERE search_request_indexer_run_id = run_id;

        RETURN;
    END IF;

    IF rate_limit_scope_input IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'rate_limit_scope_invalid';
    END IF;

    IF run_status <> 'running' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'run_invalid_state';
    END IF;

    IF error_class_input IN ('auth_error', 'http_403', 'cf_challenge', 'unknown') THEN
        UPDATE search_request_indexer_run
        SET status = 'failed',
            started_at = COALESCE(started_at, now()),
            finished_at = now(),
            next_attempt_at = NULL,
            error_class = error_class_input,
            error_detail = error_detail_input,
            last_error_class = error_class_input,
            last_rate_limit_scope = NULL
        WHERE search_request_indexer_run_id = run_id;

        RETURN;
    END IF;

    IF retry_seq_input IS NULL OR retry_seq_input < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'retry_seq_invalid';
    END IF;

    retry_seq_value := retry_seq_input;

    IF error_class_input IN ('tls', 'parse_error') THEN
        max_retry_seq := 1;
    ELSE
        max_retry_seq := 3;
    END IF;

    IF retry_seq_value >= max_retry_seq THEN
        UPDATE search_request_indexer_run
        SET status = 'failed',
            started_at = COALESCE(started_at, now()),
            finished_at = now(),
            next_attempt_at = NULL,
            error_class = error_class_input,
            error_detail = error_detail_input,
            last_error_class = error_class_input,
            last_rate_limit_scope = NULL
        WHERE search_request_indexer_run_id = run_id;

        RETURN;
    END IF;

    IF error_class_input = 'http_429' THEN
        delay_no_jitter := LEAST(30 * (1 << retry_seq_value), 600);
        IF retry_after_seconds_input IS NOT NULL AND retry_after_seconds_input > 0 THEN
            delay_no_jitter := GREATEST(delay_no_jitter, retry_after_seconds_input);
        END IF;
    ELSE
        delay_no_jitter := LEAST(2 * (1 << retry_seq_value), 120);
    END IF;

    jitter_pct := random_jitter_seconds(25);
    jitter_seconds := (delay_no_jitter * jitter_pct) / 100;
    next_attempt := now() + make_interval(secs => delay_no_jitter + jitter_seconds);

    UPDATE search_request_indexer_run
    SET status = 'queued',
        started_at = NULL,
        next_attempt_at = next_attempt,
        last_error_class = error_class_input,
        last_rate_limit_scope = NULL
    WHERE search_request_indexer_run_id = run_id;
END;
$$;

CREATE OR REPLACE FUNCTION search_indexer_run_mark_failed(
    search_request_public_id_input UUID,
    indexer_instance_public_id_input UUID,
    error_class_input error_class,
    error_detail_input VARCHAR,
    retry_after_seconds_input INTEGER,
    retry_seq_input SMALLINT,
    rate_limit_scope_input rate_limit_scope
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_indexer_run_mark_failed_v1(
        search_request_public_id_input,
        indexer_instance_public_id_input,
        error_class_input,
        error_detail_input,
        retry_after_seconds_input,
        retry_seq_input,
        rate_limit_scope_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_indexer_run_mark_canceled_v1(
    search_request_public_id_input UUID,
    indexer_instance_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to mark indexer run canceled';
    errcode CONSTANT text := 'P0001';
    request_id BIGINT;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    run_id BIGINT;
    run_status run_status;
BEGIN
    IF search_request_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_missing';
    END IF;

    IF indexer_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_missing';
    END IF;

    SELECT search_request_id
    INTO request_id
    FROM search_request
    WHERE search_request_public_id = search_request_public_id_input;

    IF request_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_not_found';
    END IF;

    SELECT indexer_instance_id, deleted_at
    INTO instance_id, instance_deleted_at
    FROM indexer_instance
    WHERE indexer_instance_public_id = indexer_instance_public_id_input;

    IF instance_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_not_found';
    END IF;

    IF instance_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_deleted';
    END IF;

    SELECT search_request_indexer_run_id, status
    INTO run_id, run_status
    FROM search_request_indexer_run
    WHERE search_request_id = request_id
      AND indexer_instance_id = instance_id;

    IF run_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'run_not_found';
    END IF;

    IF run_status IN ('finished', 'failed', 'canceled') THEN
        RETURN;
    END IF;

    UPDATE search_request_indexer_run
    SET status = 'canceled',
        started_at = COALESCE(started_at, now()),
        finished_at = now(),
        next_attempt_at = NULL,
        error_class = NULL,
        error_detail = NULL
    WHERE search_request_indexer_run_id = run_id;
END;
$$;

CREATE OR REPLACE FUNCTION search_indexer_run_mark_canceled(
    search_request_public_id_input UUID,
    indexer_instance_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_indexer_run_mark_canceled_v1(
        search_request_public_id_input,
        indexer_instance_public_id_input
    );
END;
$$;
