-- Job schedule completion updates for job runner procedures.

CREATE OR REPLACE FUNCTION job_schedule_mark_completed_v1(
    job_key_input job_key
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to update job schedule';
    errcode CONSTANT text := 'P0001';
    schedule_id BIGINT;
    cadence_seconds_value INTEGER;
    jitter_seconds_value INTEGER;
    now_ts TIMESTAMPTZ;
    jitter_value INTEGER;
BEGIN
    IF job_key_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'job_key_missing';
    END IF;

    SELECT job_schedule_id, cadence_seconds, jitter_seconds
    INTO schedule_id, cadence_seconds_value, jitter_seconds_value
    FROM job_schedule
    WHERE job_key = job_key_input;

    IF schedule_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'job_not_found';
    END IF;

    now_ts := now();
    jitter_value := random_jitter_seconds(jitter_seconds_value);

    UPDATE job_schedule
    SET last_run_at = now_ts,
        next_run_at = now_ts + make_interval(secs => cadence_seconds_value + jitter_value),
        locked_until = NULL,
        lock_owner = NULL
    WHERE job_schedule_id = schedule_id;
END;
$$;

CREATE OR REPLACE FUNCTION job_schedule_mark_completed(
    job_key_input job_key
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_schedule_mark_completed_v1(job_key_input => job_key_input);
END;
$$;

CREATE OR REPLACE FUNCTION job_run_retention_purge_v2()
RETURNS TABLE (
    ok BOOLEAN,
    error_code TEXT,
    error_detail TEXT
)
LANGUAGE plpgsql
AS $$
DECLARE
    ok_value BOOLEAN := TRUE;
    error_code_value TEXT;
    error_detail_value TEXT;
BEGIN
    BEGIN
        PERFORM job_run_retention_purge_v1();
    EXCEPTION WHEN OTHERS THEN
        ok_value := FALSE;
        GET STACKED DIAGNOSTICS
            error_code_value = RETURNED_SQLSTATE,
            error_detail_value = PG_EXCEPTION_DETAIL;
    END;

    PERFORM job_schedule_mark_completed_v1('retention_purge');

    RETURN QUERY SELECT ok_value, error_code_value, error_detail_value;
END;
$$;

CREATE OR REPLACE FUNCTION job_run_connectivity_profile_refresh_v2()
RETURNS TABLE (
    ok BOOLEAN,
    error_code TEXT,
    error_detail TEXT
)
LANGUAGE plpgsql
AS $$
DECLARE
    ok_value BOOLEAN := TRUE;
    error_code_value TEXT;
    error_detail_value TEXT;
BEGIN
    BEGIN
        PERFORM job_run_connectivity_profile_refresh_v1();
    EXCEPTION WHEN OTHERS THEN
        ok_value := FALSE;
        GET STACKED DIAGNOSTICS
            error_code_value = RETURNED_SQLSTATE,
            error_detail_value = PG_EXCEPTION_DETAIL;
    END;

    PERFORM job_schedule_mark_completed_v1('connectivity_profile_refresh');

    RETURN QUERY SELECT ok_value, error_code_value, error_detail_value;
END;
$$;

CREATE OR REPLACE FUNCTION job_run_reputation_rollup_v2(
    window_key_input reputation_window
)
RETURNS TABLE (
    ok BOOLEAN,
    error_code TEXT,
    error_detail TEXT
)
LANGUAGE plpgsql
AS $$
DECLARE
    job_key_value job_key;
    ok_value BOOLEAN := TRUE;
    error_code_value TEXT;
    error_detail_value TEXT;
BEGIN
    job_key_value := CASE window_key_input
        WHEN '1h' THEN 'reputation_rollup_1h'::job_key
        WHEN '24h' THEN 'reputation_rollup_24h'::job_key
        WHEN '7d' THEN 'reputation_rollup_7d'::job_key
    END;

    BEGIN
        PERFORM job_run_reputation_rollup_v1(window_key_input => window_key_input);
    EXCEPTION WHEN OTHERS THEN
        ok_value := FALSE;
        GET STACKED DIAGNOSTICS
            error_code_value = RETURNED_SQLSTATE,
            error_detail_value = PG_EXCEPTION_DETAIL;
    END;

    PERFORM job_schedule_mark_completed_v1(job_key_value);

    RETURN QUERY SELECT ok_value, error_code_value, error_detail_value;
END;
$$;

CREATE OR REPLACE FUNCTION job_run_canonical_backfill_best_source_v2()
RETURNS TABLE (
    ok BOOLEAN,
    error_code TEXT,
    error_detail TEXT
)
LANGUAGE plpgsql
AS $$
DECLARE
    ok_value BOOLEAN := TRUE;
    error_code_value TEXT;
    error_detail_value TEXT;
BEGIN
    BEGIN
        PERFORM job_run_canonical_backfill_best_source_v1();
    EXCEPTION WHEN OTHERS THEN
        ok_value := FALSE;
        GET STACKED DIAGNOSTICS
            error_code_value = RETURNED_SQLSTATE,
            error_detail_value = PG_EXCEPTION_DETAIL;
    END;

    PERFORM job_schedule_mark_completed_v1('canonical_backfill_best_source');

    RETURN QUERY SELECT ok_value, error_code_value, error_detail_value;
END;
$$;

CREATE OR REPLACE FUNCTION job_run_base_score_refresh_recent_v2()
RETURNS TABLE (
    ok BOOLEAN,
    error_code TEXT,
    error_detail TEXT
)
LANGUAGE plpgsql
AS $$
DECLARE
    ok_value BOOLEAN := TRUE;
    error_code_value TEXT;
    error_detail_value TEXT;
BEGIN
    BEGIN
        PERFORM job_run_base_score_refresh_recent_v1();
    EXCEPTION WHEN OTHERS THEN
        ok_value := FALSE;
        GET STACKED DIAGNOSTICS
            error_code_value = RETURNED_SQLSTATE,
            error_detail_value = PG_EXCEPTION_DETAIL;
    END;

    PERFORM job_schedule_mark_completed_v1('base_score_refresh_recent');

    RETURN QUERY SELECT ok_value, error_code_value, error_detail_value;
END;
$$;

CREATE OR REPLACE FUNCTION job_run_rss_subscription_backfill_v2()
RETURNS TABLE (
    ok BOOLEAN,
    error_code TEXT,
    error_detail TEXT
)
LANGUAGE plpgsql
AS $$
DECLARE
    ok_value BOOLEAN := TRUE;
    error_code_value TEXT;
    error_detail_value TEXT;
BEGIN
    BEGIN
        PERFORM job_run_rss_subscription_backfill_v1();
    EXCEPTION WHEN OTHERS THEN
        ok_value := FALSE;
        GET STACKED DIAGNOSTICS
            error_code_value = RETURNED_SQLSTATE,
            error_detail_value = PG_EXCEPTION_DETAIL;
    END;

    PERFORM job_schedule_mark_completed_v1('rss_subscription_backfill');

    RETURN QUERY SELECT ok_value, error_code_value, error_detail_value;
END;
$$;

CREATE OR REPLACE FUNCTION job_run_policy_snapshot_gc_v2()
RETURNS TABLE (
    ok BOOLEAN,
    error_code TEXT,
    error_detail TEXT
)
LANGUAGE plpgsql
AS $$
DECLARE
    ok_value BOOLEAN := TRUE;
    error_code_value TEXT;
    error_detail_value TEXT;
BEGIN
    BEGIN
        PERFORM job_run_policy_snapshot_gc_v1();
    EXCEPTION WHEN OTHERS THEN
        ok_value := FALSE;
        GET STACKED DIAGNOSTICS
            error_code_value = RETURNED_SQLSTATE,
            error_detail_value = PG_EXCEPTION_DETAIL;
    END;

    PERFORM job_schedule_mark_completed_v1('policy_snapshot_gc');

    RETURN QUERY SELECT ok_value, error_code_value, error_detail_value;
END;
$$;

CREATE OR REPLACE FUNCTION job_run_policy_snapshot_refcount_repair_v2()
RETURNS TABLE (
    ok BOOLEAN,
    error_code TEXT,
    error_detail TEXT
)
LANGUAGE plpgsql
AS $$
DECLARE
    ok_value BOOLEAN := TRUE;
    error_code_value TEXT;
    error_detail_value TEXT;
BEGIN
    BEGIN
        PERFORM job_run_policy_snapshot_refcount_repair_v1();
    EXCEPTION WHEN OTHERS THEN
        ok_value := FALSE;
        GET STACKED DIAGNOSTICS
            error_code_value = RETURNED_SQLSTATE,
            error_detail_value = PG_EXCEPTION_DETAIL;
    END;

    PERFORM job_schedule_mark_completed_v1('policy_snapshot_refcount_repair');

    RETURN QUERY SELECT ok_value, error_code_value, error_detail_value;
END;
$$;

CREATE OR REPLACE FUNCTION job_run_rate_limit_state_purge_v2()
RETURNS TABLE (
    ok BOOLEAN,
    error_code TEXT,
    error_detail TEXT
)
LANGUAGE plpgsql
AS $$
DECLARE
    ok_value BOOLEAN := TRUE;
    error_code_value TEXT;
    error_detail_value TEXT;
BEGIN
    BEGIN
        PERFORM job_run_rate_limit_state_purge_v1();
    EXCEPTION WHEN OTHERS THEN
        ok_value := FALSE;
        GET STACKED DIAGNOSTICS
            error_code_value = RETURNED_SQLSTATE,
            error_detail_value = PG_EXCEPTION_DETAIL;
    END;

    PERFORM job_schedule_mark_completed_v1('rate_limit_state_purge');

    RETURN QUERY SELECT ok_value, error_code_value, error_detail_value;
END;
$$;

CREATE OR REPLACE FUNCTION job_run_retention_purge()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_retention_purge_v2();
END;
$$;

CREATE OR REPLACE FUNCTION job_run_connectivity_profile_refresh()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_connectivity_profile_refresh_v2();
END;
$$;

CREATE OR REPLACE FUNCTION job_run_reputation_rollup(
    window_key_input reputation_window
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_reputation_rollup_v2(window_key_input => window_key_input);
END;
$$;

CREATE OR REPLACE FUNCTION job_run_canonical_backfill_best_source()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_canonical_backfill_best_source_v2();
END;
$$;

CREATE OR REPLACE FUNCTION job_run_base_score_refresh_recent()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_base_score_refresh_recent_v2();
END;
$$;

CREATE OR REPLACE FUNCTION job_run_rss_subscription_backfill()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_rss_subscription_backfill_v2();
END;
$$;

CREATE OR REPLACE FUNCTION job_run_policy_snapshot_gc()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_policy_snapshot_gc_v2();
END;
$$;

CREATE OR REPLACE FUNCTION job_run_policy_snapshot_refcount_repair()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_policy_snapshot_refcount_repair_v2();
END;
$$;

CREATE OR REPLACE FUNCTION job_run_rate_limit_state_purge()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_rate_limit_state_purge_v2();
END;
$$;
