-- Outbound request log write procedure.

CREATE OR REPLACE FUNCTION outbound_request_log_write_v1(
    indexer_instance_public_id_input UUID,
    routing_policy_public_id_input UUID,
    search_request_public_id_input UUID,
    request_type_input outbound_request_type,
    correlation_id_input UUID,
    retry_seq_input SMALLINT,
    started_at_input TIMESTAMPTZ,
    finished_at_input TIMESTAMPTZ,
    outcome_input outbound_request_outcome,
    via_mitigation_input outbound_via_mitigation,
    rate_limit_denied_scope_input rate_limit_scope,
    error_class_input error_class,
    http_status_input INTEGER,
    latency_ms_input INTEGER,
    parse_ok_input BOOLEAN,
    result_count_input INTEGER,
    cf_detected_input BOOLEAN,
    page_number_input INTEGER,
    page_cursor_key_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to write outbound request log';
    errcode CONSTANT text := 'P0001';
    indexer_instance_id_value BIGINT;
    indexer_instance_deleted_at TIMESTAMPTZ;
    routing_policy_id_value BIGINT;
    routing_policy_deleted_at TIMESTAMPTZ;
    search_request_id_value BIGINT;
    search_request_run_id_value BIGINT;
    trimmed_cursor TEXT;
    normalized_cursor TEXT;
    cursor_is_hashed BOOLEAN := FALSE;
    url_match TEXT[];
    url_scheme TEXT;
    url_host TEXT;
    url_path TEXT;
    url_query TEXT;
    normalized_query TEXT;
    normalized_candidate TEXT;
    hash_hex TEXT;
BEGIN
    IF indexer_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_missing';
    END IF;

    SELECT indexer_instance_id, deleted_at
    INTO indexer_instance_id_value, indexer_instance_deleted_at
    FROM indexer_instance
    WHERE indexer_instance_public_id = indexer_instance_public_id_input;

    IF indexer_instance_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_not_found';
    END IF;

    IF indexer_instance_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_deleted';
    END IF;

    routing_policy_id_value := NULL;
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

    search_request_id_value := NULL;
    IF search_request_public_id_input IS NOT NULL THEN
        SELECT search_request_id
        INTO search_request_id_value
        FROM search_request
        WHERE search_request_public_id = search_request_public_id_input;

        IF search_request_id_value IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'search_request_not_found';
        END IF;

        SELECT search_request_indexer_run_id
        INTO search_request_run_id_value
        FROM search_request_indexer_run
        WHERE search_request_id = search_request_id_value
          AND indexer_instance_id = indexer_instance_id_value;

        IF search_request_run_id_value IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'search_run_not_found';
        END IF;
    END IF;

    IF request_type_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'request_type_missing';
    END IF;

    IF correlation_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'correlation_id_missing';
    END IF;

    IF retry_seq_input IS NULL OR retry_seq_input < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'retry_seq_invalid';
    END IF;

    IF started_at_input IS NULL OR finished_at_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'timestamp_missing';
    END IF;

    IF outcome_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'outcome_missing';
    END IF;

    IF via_mitigation_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'via_mitigation_missing';
    END IF;

    IF parse_ok_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'parse_ok_missing';
    END IF;

    IF cf_detected_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'cf_detected_missing';
    END IF;

    IF latency_ms_input IS NOT NULL AND latency_ms_input < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'latency_invalid';
    END IF;

    IF result_count_input IS NOT NULL AND result_count_input < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'result_count_invalid';
    END IF;

    IF page_number_input IS NOT NULL AND page_number_input < 1 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'page_number_invalid';
    END IF;

    IF outcome_input = 'success' THEN
        IF parse_ok_input IS DISTINCT FROM TRUE THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'parse_ok_required';
        END IF;
        IF error_class_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'error_class_not_allowed';
        END IF;
        IF request_type_input <> 'probe' AND result_count_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'result_count_missing';
        END IF;
        IF rate_limit_denied_scope_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'rate_limit_scope_invalid';
        END IF;
    ELSE
        IF error_class_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'error_class_missing';
        END IF;
    END IF;

    IF cf_detected_input = TRUE AND outcome_input = 'failure' THEN
        IF error_class_input IS DISTINCT FROM 'cf_challenge' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'error_class_invalid';
        END IF;
    END IF;

    IF error_class_input = 'rate_limited' THEN
        IF rate_limit_denied_scope_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'rate_limit_scope_missing';
        END IF;
        IF result_count_input IS DISTINCT FROM 0 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'result_count_invalid';
        END IF;
    ELSE
        IF rate_limit_denied_scope_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'rate_limit_scope_invalid';
        END IF;
        IF outcome_input = 'failure' AND result_count_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'result_count_not_allowed';
        END IF;
    END IF;

    normalized_cursor := NULL;
    cursor_is_hashed := FALSE;
    IF page_cursor_key_input IS NOT NULL THEN
        trimmed_cursor := btrim(page_cursor_key_input);
        IF trimmed_cursor = '' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'page_cursor_invalid';
        END IF;

        url_match := regexp_match(
            trimmed_cursor,
            '^(https?)://([^/?#]+)([^?#]*)([?][^#]*)?(#.*)?$'
        );

        IF url_match IS NOT NULL THEN
            url_scheme := lower(url_match[1]);
            url_host := lower(url_match[2]);
            url_path := COALESCE(url_match[3], '');
            url_query := NULL;
            IF array_length(url_match, 1) >= 4 THEN
                url_query := url_match[4];
            END IF;

            IF url_query IS NOT NULL THEN
                url_query := substring(url_query from 2);
            END IF;

            IF url_query IS NULL OR url_query = '' THEN
                normalized_cursor := url_scheme || '://' || url_host || url_path;
            ELSE
                SELECT string_agg(param, '&' ORDER BY key, value)
                INTO normalized_query
                FROM (
                    SELECT
                        CASE
                            WHEN position('=' in part) > 0 THEN split_part(part, '=', 1)
                            ELSE part
                        END AS key,
                        CASE
                            WHEN position('=' in part) > 0 THEN substring(part from position('=' in part) + 1)
                            ELSE ''
                        END AS value,
                        part AS param
                    FROM unnest(string_to_array(url_query, '&')) AS part
                ) AS parts;

                normalized_cursor := url_scheme || '://' || url_host || url_path || '?' || normalized_query;
            END IF;
        ELSE
            normalized_cursor := trimmed_cursor;
        END IF;

        normalized_candidate := normalized_cursor;
        IF char_length(normalized_candidate) > 64 THEN
            hash_hex := encode(digest(normalized_candidate, 'sha256'), 'hex');
            normalized_cursor := substring(hash_hex from 1 for 16);
            cursor_is_hashed := TRUE;
        END IF;
    END IF;

    INSERT INTO outbound_request_log (
        indexer_instance_id,
        routing_policy_id,
        search_request_id,
        request_type,
        correlation_id,
        retry_seq,
        started_at,
        finished_at,
        outcome,
        via_mitigation,
        rate_limit_denied_scope,
        error_class,
        http_status,
        latency_ms,
        parse_ok,
        result_count,
        cf_detected,
        page_number,
        page_cursor_key,
        page_cursor_is_hashed
    )
    VALUES (
        indexer_instance_id_value,
        routing_policy_id_value,
        search_request_id_value,
        request_type_input,
        correlation_id_input,
        retry_seq_input,
        started_at_input,
        finished_at_input,
        outcome_input,
        via_mitigation_input,
        rate_limit_denied_scope_input,
        error_class_input,
        http_status_input,
        latency_ms_input,
        parse_ok_input,
        result_count_input,
        cf_detected_input,
        page_number_input,
        normalized_cursor,
        cursor_is_hashed
    );

    IF search_request_run_id_value IS NOT NULL THEN
        UPDATE search_request_indexer_run
        SET last_correlation_id = correlation_id_input
        WHERE search_request_indexer_run_id = search_request_run_id_value;

        INSERT INTO search_request_indexer_run_correlation (
            search_request_indexer_run_id,
            correlation_id,
            page_number
        )
        VALUES (
            search_request_run_id_value,
            correlation_id_input,
            page_number_input
        )
        ON CONFLICT (search_request_indexer_run_id, correlation_id) DO NOTHING;
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION outbound_request_log_write(
    indexer_instance_public_id_input UUID,
    routing_policy_public_id_input UUID,
    search_request_public_id_input UUID,
    request_type_input outbound_request_type,
    correlation_id_input UUID,
    retry_seq_input SMALLINT,
    started_at_input TIMESTAMPTZ,
    finished_at_input TIMESTAMPTZ,
    outcome_input outbound_request_outcome,
    via_mitigation_input outbound_via_mitigation,
    rate_limit_denied_scope_input rate_limit_scope,
    error_class_input error_class,
    http_status_input INTEGER,
    latency_ms_input INTEGER,
    parse_ok_input BOOLEAN,
    result_count_input INTEGER,
    cf_detected_input BOOLEAN,
    page_number_input INTEGER,
    page_cursor_key_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM outbound_request_log_write_v1(
        indexer_instance_public_id_input,
        routing_policy_public_id_input,
        search_request_public_id_input,
        request_type_input,
        correlation_id_input,
        retry_seq_input,
        started_at_input,
        finished_at_input,
        outcome_input,
        via_mitigation_input,
        rate_limit_denied_scope_input,
        error_class_input,
        http_status_input,
        latency_ms_input,
        parse_ok_input,
        result_count_input,
        cf_detected_input,
        page_number_input,
        page_cursor_key_input
    );
END;
$$;
