-- Additional job runner procedures.

CREATE OR REPLACE FUNCTION job_run_policy_snapshot_gc_v1()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    DELETE FROM policy_snapshot
    WHERE ref_count = 0
      AND created_at < now() - make_interval(days => 30);
END;
$$;

CREATE OR REPLACE FUNCTION job_run_policy_snapshot_gc()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_policy_snapshot_gc_v1();
END;
$$;

CREATE OR REPLACE FUNCTION job_run_policy_snapshot_refcount_repair_v1()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    WITH counts AS (
        SELECT policy_snapshot_id, COUNT(*) AS request_count
        FROM search_request
        GROUP BY policy_snapshot_id
    )
    UPDATE policy_snapshot
    SET ref_count = counts.request_count
    FROM counts
    WHERE policy_snapshot.policy_snapshot_id = counts.policy_snapshot_id
      AND policy_snapshot.ref_count IS DISTINCT FROM counts.request_count;

    UPDATE policy_snapshot
    SET ref_count = 0
    WHERE NOT EXISTS (
        SELECT 1
        FROM search_request
        WHERE search_request.policy_snapshot_id = policy_snapshot.policy_snapshot_id
    )
      AND ref_count <> 0;
END;
$$;

CREATE OR REPLACE FUNCTION job_run_policy_snapshot_refcount_repair()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_policy_snapshot_refcount_repair_v1();
END;
$$;

CREATE OR REPLACE FUNCTION job_run_rate_limit_state_purge_v1()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    DELETE FROM rate_limit_state
    WHERE window_start < now() - make_interval(hours => 6);
END;
$$;

CREATE OR REPLACE FUNCTION job_run_rate_limit_state_purge()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_rate_limit_state_purge_v1();
END;
$$;
