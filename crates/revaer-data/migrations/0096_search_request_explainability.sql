-- Search request explainability summary for zero-result diagnostics.

CREATE OR REPLACE FUNCTION search_request_explainability_v1(
    actor_user_public_id UUID,
    search_request_public_id_input UUID
)
RETURNS TABLE(
    zero_runnable_indexers BOOLEAN,
    skipped_canceled_indexers INTEGER,
    skipped_failed_indexers INTEGER,
    blocked_results INTEGER,
    blocked_rule_public_ids UUID[],
    rate_limited_indexers INTEGER,
    retrying_indexers INTEGER
)
LANGUAGE plpgsql
AS $$
DECLARE
    request_id BIGINT;
BEGIN
    -- Reuse list auth/visibility checks so error detail codes remain consistent.
    PERFORM *
    FROM search_page_list_v1(
        actor_user_public_id => actor_user_public_id,
        search_request_public_id_input => search_request_public_id_input
    )
    LIMIT 1;

    SELECT search_request_id
    INTO request_id
    FROM search_request
    WHERE search_request_public_id = search_request_public_id_input;

    RETURN QUERY
    WITH run_counts AS (
        SELECT
            COUNT(*)::INTEGER AS runnable_count,
            COUNT(*) FILTER (WHERE status = 'canceled')::INTEGER AS canceled_count,
            COUNT(*) FILTER (WHERE status = 'failed')::INTEGER AS failed_count,
            COUNT(*) FILTER (WHERE last_error_class = 'rate_limited')::INTEGER AS rate_limited_count,
            COUNT(*) FILTER (
                WHERE status = 'queued'
                  AND next_attempt_at IS NOT NULL
                  AND next_attempt_at > now()
            )::INTEGER AS retrying_count
        FROM search_request_indexer_run
        WHERE search_request_id = request_id
    ), block_counts AS (
        SELECT
            COUNT(*)::INTEGER AS blocked_count,
            COALESCE(
                array_agg(DISTINCT policy_rule_public_id ORDER BY policy_rule_public_id),
                ARRAY[]::UUID[]
            ) AS blocked_rule_ids
        FROM search_filter_decision
        WHERE search_request_id = request_id
          AND decision IN ('drop_source', 'drop_canonical')
    )
    SELECT
        (run_counts.runnable_count = 0) AS zero_runnable_indexers,
        run_counts.canceled_count,
        run_counts.failed_count,
        block_counts.blocked_count,
        block_counts.blocked_rule_ids,
        run_counts.rate_limited_count,
        run_counts.retrying_count
    FROM run_counts
    CROSS JOIN block_counts;
END;
$$;

CREATE OR REPLACE FUNCTION search_request_explainability(
    actor_user_public_id UUID,
    search_request_public_id_input UUID
)
RETURNS TABLE(
    zero_runnable_indexers BOOLEAN,
    skipped_canceled_indexers INTEGER,
    skipped_failed_indexers INTEGER,
    blocked_results INTEGER,
    blocked_rule_public_ids UUID[],
    rate_limited_indexers INTEGER,
    retrying_indexers INTEGER
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT * FROM search_request_explainability_v1(
        actor_user_public_id => actor_user_public_id,
        search_request_public_id_input => search_request_public_id_input
    );
END;
$$;
