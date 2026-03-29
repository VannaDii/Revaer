-- Job runner procedures.

CREATE OR REPLACE FUNCTION job_claim_next_v1(
    job_key_input job_key
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to claim job';
    errcode CONSTANT text := 'P0001';
    schedule_id BIGINT;
    schedule_enabled BOOLEAN;
    schedule_next_run TIMESTAMPTZ;
    schedule_locked_until TIMESTAMPTZ;
    lease_seconds INTEGER;
    now_ts TIMESTAMPTZ;
    lock_acquired BOOLEAN;
BEGIN
    IF job_key_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'job_key_missing';
    END IF;

    SELECT job_schedule_id, enabled, next_run_at, locked_until
    INTO schedule_id, schedule_enabled, schedule_next_run, schedule_locked_until
    FROM job_schedule
    WHERE job_key = job_key_input;

    IF schedule_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'job_not_found';
    END IF;

    IF schedule_enabled IS NOT TRUE THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'job_disabled';
    END IF;

    now_ts := now();

    IF schedule_next_run > now_ts THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'job_not_due';
    END IF;

    IF schedule_locked_until IS NOT NULL AND schedule_locked_until > now_ts THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'job_locked';
    END IF;

    lock_acquired := pg_try_advisory_xact_lock(hashtext(job_key_input::text)::BIGINT);
    IF lock_acquired IS NOT TRUE THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'job_locked';
    END IF;

    lease_seconds := CASE job_key_input
        WHEN 'connectivity_profile_refresh' THEN 30
        WHEN 'reputation_rollup_1h' THEN 60
        WHEN 'reputation_rollup_24h' THEN 300
        WHEN 'reputation_rollup_7d' THEN 600
        WHEN 'retention_purge' THEN 300
        WHEN 'canonical_backfill_best_source' THEN 900
        WHEN 'base_score_refresh_recent' THEN 900
        WHEN 'canonical_prune_low_confidence' THEN 900
        WHEN 'policy_snapshot_gc' THEN 900
        WHEN 'policy_snapshot_refcount_repair' THEN 900
        WHEN 'rate_limit_state_purge' THEN 300
        WHEN 'rss_poll' THEN 60
        WHEN 'rss_subscription_backfill' THEN 300
        ELSE 300
    END;

    UPDATE job_schedule
    SET locked_until = now_ts + make_interval(secs => lease_seconds),
        lock_owner = current_user
    WHERE job_schedule_id = schedule_id;
END;
$$;

CREATE OR REPLACE FUNCTION job_claim_next(
    job_key_input job_key
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_claim_next_v1(job_key_input);
END;
$$;

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

CREATE OR REPLACE FUNCTION job_run_retention_purge()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_retention_purge_v1();
END;
$$;
