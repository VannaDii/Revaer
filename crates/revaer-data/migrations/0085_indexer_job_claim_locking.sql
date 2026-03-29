-- Job claim locking and lease duration updates.

CREATE OR REPLACE FUNCTION job_claim_lease_seconds_v1(
    job_key_input job_key
)
RETURNS INTEGER
LANGUAGE sql
IMMUTABLE
STRICT
AS $$
    SELECT CASE job_key_input
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
$$;

CREATE OR REPLACE FUNCTION job_claim_lease_seconds(
    job_key_input job_key
)
RETURNS INTEGER
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN job_claim_lease_seconds_v1(job_key_input => job_key_input);
END;
$$;

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
    now_ts TIMESTAMPTZ;
    lock_acquired BOOLEAN;
BEGIN
    IF job_key_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'job_key_missing';
    END IF;

    lock_acquired := pg_try_advisory_xact_lock(hashtext(job_key_input::text)::BIGINT);
    IF lock_acquired IS NOT TRUE THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'job_locked';
    END IF;

    SELECT job_schedule_id, enabled, next_run_at, locked_until
    INTO schedule_id, schedule_enabled, schedule_next_run, schedule_locked_until
    FROM job_schedule
    WHERE job_key = job_key_input
    FOR UPDATE;

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

    UPDATE job_schedule
    SET locked_until = now_ts + make_interval(secs => job_claim_lease_seconds_v1(job_key_input)),
        lock_owner = current_user
    WHERE job_schedule_id = schedule_id;
END;
$$;
