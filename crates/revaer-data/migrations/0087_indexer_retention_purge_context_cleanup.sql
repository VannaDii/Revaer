-- Retention purge must remove search_request-scoped context score rows.

CREATE OR REPLACE FUNCTION job_run_retention_purge_v1()
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    errcode CONSTANT text := 'P0001';
    retention_search_days_value INTEGER;
    retention_outbound_days_value INTEGER;
    retention_rss_days_value INTEGER;
    retention_conflict_days_value INTEGER;
    retention_conflict_audit_days_value INTEGER;
    retention_health_days_value INTEGER;
    retention_reputation_days_value INTEGER;
    cutoff_search TIMESTAMPTZ;
    cutoff_outbound TIMESTAMPTZ;
    cutoff_rss TIMESTAMPTZ;
    cutoff_conflict TIMESTAMPTZ;
    cutoff_conflict_audit TIMESTAMPTZ;
    cutoff_health TIMESTAMPTZ;
    cutoff_reputation TIMESTAMPTZ;
BEGIN
    SELECT retention_search_days,
           retention_outbound_request_log_days,
           retention_rss_item_seen_days,
           retention_source_metadata_conflict_days,
           retention_source_metadata_conflict_audit_days,
           retention_health_events_days,
           retention_reputation_days
    INTO retention_search_days_value,
         retention_outbound_days_value,
         retention_rss_days_value,
         retention_conflict_days_value,
         retention_conflict_audit_days_value,
         retention_health_days_value,
         retention_reputation_days_value
    FROM deployment_config
    ORDER BY deployment_config_id
    LIMIT 1;

    IF retention_search_days_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = 'Failed to purge retention data',
            DETAIL = 'deployment_config_missing';
    END IF;

    cutoff_search := now() - make_interval(days => retention_search_days_value);
    cutoff_outbound := now() - make_interval(days => retention_outbound_days_value);
    cutoff_rss := now() - make_interval(days => retention_rss_days_value);
    cutoff_conflict := now() - make_interval(days => retention_conflict_days_value);
    cutoff_conflict_audit := now() - make_interval(days => retention_conflict_audit_days_value);
    cutoff_health := now() - make_interval(days => retention_health_days_value);
    cutoff_reputation := now() - make_interval(days => retention_reputation_days_value);

    WITH purged_requests AS (
        DELETE FROM search_request
        WHERE finished_at IS NOT NULL
          AND finished_at < cutoff_search
        RETURNING search_request_id, policy_snapshot_id
    ),
    policy_counts AS (
        SELECT policy_snapshot_id, COUNT(*) AS request_count
        FROM purged_requests
        GROUP BY policy_snapshot_id
    ),
    policy_updates AS (
        UPDATE policy_snapshot
        SET ref_count = GREATEST(ref_count - policy_counts.request_count, 0)
        FROM policy_counts
        WHERE policy_snapshot.policy_snapshot_id = policy_counts.policy_snapshot_id
        RETURNING policy_snapshot.policy_snapshot_id
    ),
    purged_context_scores AS (
        DELETE FROM canonical_torrent_source_context_score
        USING purged_requests
        WHERE canonical_torrent_source_context_score.context_key_type = 'search_request'
          AND canonical_torrent_source_context_score.context_key_id = purged_requests.search_request_id
        RETURNING canonical_torrent_source_context_score_id
    ),
    purged_best_context AS (
        DELETE FROM canonical_torrent_best_source_context
        USING purged_requests
        WHERE canonical_torrent_best_source_context.context_key_type = 'search_request'
          AND canonical_torrent_best_source_context.context_key_id = purged_requests.search_request_id
        RETURNING canonical_torrent_best_source_context_id
    )
    DELETE FROM policy_set
    USING purged_requests
    WHERE policy_set.is_auto_created = TRUE
      AND policy_set.created_for_search_request_id = purged_requests.search_request_id;

    DELETE FROM outbound_request_log
    WHERE COALESCE(finished_at, started_at) < cutoff_outbound;

    DELETE FROM indexer_rss_item_seen
    WHERE first_seen_at < cutoff_rss;

    DELETE FROM source_metadata_conflict
    WHERE observed_at < cutoff_conflict;

    DELETE FROM source_metadata_conflict_audit_log
    WHERE occurred_at < cutoff_conflict_audit;

    DELETE FROM indexer_health_event
    WHERE occurred_at < cutoff_health;

    DELETE FROM source_reputation
    WHERE window_start < cutoff_reputation;
END;
$$;
