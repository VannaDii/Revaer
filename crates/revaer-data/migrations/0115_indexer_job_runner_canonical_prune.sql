-- Add canonical prune wrapper that updates job schedule state.

CREATE OR REPLACE FUNCTION job_run_canonical_prune_low_confidence_v2()
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
        PERFORM canonical_prune_low_confidence_v1();
    EXCEPTION WHEN OTHERS THEN
        ok_value := FALSE;
        GET STACKED DIAGNOSTICS
            error_code_value = RETURNED_SQLSTATE,
            error_detail_value = PG_EXCEPTION_DETAIL;
    END;

    PERFORM job_schedule_mark_completed_v1('canonical_prune_low_confidence');

    RETURN QUERY SELECT ok_value, error_code_value, error_detail_value;
END;
$$;

CREATE OR REPLACE FUNCTION job_run_canonical_prune_low_confidence()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_canonical_prune_low_confidence_v2();
END;
$$;
