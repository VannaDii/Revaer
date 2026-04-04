-- Enforce policy snapshot refcount repair before GC.

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
        PERFORM job_run_policy_snapshot_refcount_repair_v1();
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
