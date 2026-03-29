-- Align source reputation rollups with ERD sample thresholds and sample semantics.

CREATE OR REPLACE FUNCTION job_run_reputation_rollup_v1(
    window_key_input reputation_window
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to roll up reputation';
    errcode CONSTANT text := 'P0001';
    window_key_value reputation_window;
    window_start_value TIMESTAMPTZ;
    window_cutoff TIMESTAMPTZ;
    now_ts TIMESTAMPTZ;
BEGIN
    IF window_key_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'window_missing';
    END IF;

    window_key_value := window_key_input;
    now_ts := now();

    IF window_key_value = '1h' THEN
        window_start_value := date_trunc('hour', now_ts);
        window_cutoff := now_ts - make_interval(hours => 1);
    ELSIF window_key_value = '24h' THEN
        window_start_value := date_trunc('hour', now_ts);
        window_cutoff := now_ts - make_interval(hours => 24);
    ELSIF window_key_value = '7d' THEN
        window_start_value := date_trunc('day', now_ts);
        window_cutoff := now_ts - make_interval(days => 7);
    ELSE
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'window_invalid';
    END IF;

    WITH indexer_scope AS (
        SELECT indexer_instance_id
        FROM indexer_instance
        WHERE deleted_at IS NULL
    ),
    request_stats AS (
        SELECT indexer_instance_id,
               COUNT(*) AS request_count,
               COUNT(*) FILTER (WHERE outcome = 'success' AND parse_ok = TRUE) AS request_success_count
        FROM outbound_request_log
        WHERE request_type IN ('caps', 'search', 'tvsearch', 'moviesearch', 'rss', 'probe')
          AND finished_at >= window_cutoff
          AND error_class IS DISTINCT FROM 'rate_limited'
        GROUP BY indexer_instance_id
    ),
    acquisition_stats AS (
        SELECT src.indexer_instance_id,
               COUNT(*) AS acquisition_count,
               COUNT(*) FILTER (WHERE a.status = 'succeeded') AS acquisition_success_count,
               COUNT(*) FILTER (WHERE a.failure_class = 'dmca') AS dmca_count,
               COUNT(*) FILTER (WHERE a.failure_class IN ('corrupted', 'passworded')) AS fake_failure_count
        FROM acquisition_attempt a
        JOIN canonical_torrent_source src
            ON src.canonical_torrent_source_id = a.canonical_torrent_source_id
        JOIN indexer_scope s
            ON s.indexer_instance_id = src.indexer_instance_id
        WHERE a.started_at >= window_cutoff
        GROUP BY src.indexer_instance_id
    ),
    fake_reports AS (
        SELECT src.indexer_instance_id,
               COUNT(*) AS fake_report_count
        FROM user_result_action ura
        JOIN user_result_action_kv kv
            ON kv.user_result_action_id = ura.user_result_action_id
           AND kv.key = 'chosen_source_public_id'
        JOIN canonical_torrent_source src
            ON src.canonical_torrent_source_public_id::TEXT = kv.value
        JOIN indexer_scope s
            ON s.indexer_instance_id = src.indexer_instance_id
        WHERE ura.created_at >= window_cutoff
          AND ura.action = 'reported_fake'
        GROUP BY src.indexer_instance_id
    ),
    combined AS (
        SELECT s.indexer_instance_id,
               COALESCE(r.request_count, 0) AS request_count,
               COALESCE(r.request_success_count, 0) AS request_success_count,
               COALESCE(a.acquisition_count, 0) AS acquisition_count,
               COALESCE(a.acquisition_success_count, 0) AS acquisition_success_count,
               COALESCE(a.dmca_count, 0) AS dmca_count,
               COALESCE(a.fake_failure_count, 0) AS fake_failure_count,
               COALESCE(f.fake_report_count, 0) AS fake_report_count
        FROM indexer_scope s
        LEFT JOIN request_stats r
            ON r.indexer_instance_id = s.indexer_instance_id
        LEFT JOIN acquisition_stats a
            ON a.indexer_instance_id = s.indexer_instance_id
        LEFT JOIN fake_reports f
            ON f.indexer_instance_id = s.indexer_instance_id
    ),
    eligible AS (
        SELECT c.*
        FROM combined c
        WHERE c.request_count >= 30
           OR c.acquisition_count >= 10
    )
    INSERT INTO source_reputation (
        indexer_instance_id,
        window_key,
        window_start,
        request_success_rate,
        acquisition_success_rate,
        fake_rate,
        dmca_rate,
        request_count,
        request_success_count,
        acquisition_count,
        acquisition_success_count,
        min_samples,
        computed_at
    )
    SELECT e.indexer_instance_id,
           window_key_value,
           window_start_value,
           CASE
               WHEN e.request_count > 0 THEN e.request_success_count::NUMERIC / e.request_count
               ELSE 0
           END AS request_success_rate,
           CASE
               WHEN e.acquisition_count > 0 THEN e.acquisition_success_count::NUMERIC / e.acquisition_count
               ELSE 0
           END AS acquisition_success_rate,
           CASE
               WHEN e.acquisition_count > 0 THEN
                   LEAST(e.fake_failure_count + e.fake_report_count, e.acquisition_count)::NUMERIC / e.acquisition_count
               ELSE 0
           END AS fake_rate,
           CASE
               WHEN e.acquisition_count > 0 THEN e.dmca_count::NUMERIC / e.acquisition_count
               ELSE 0
           END AS dmca_rate,
           e.request_count,
           e.request_success_count,
           e.acquisition_count,
           e.acquisition_success_count,
           CASE
               WHEN e.acquisition_count >= 10 THEN 10
               ELSE 30
           END AS min_samples,
           now_ts
    FROM eligible e
    ON CONFLICT (indexer_instance_id, window_key, window_start)
    DO UPDATE SET
        request_success_rate = EXCLUDED.request_success_rate,
        acquisition_success_rate = EXCLUDED.acquisition_success_rate,
        fake_rate = EXCLUDED.fake_rate,
        dmca_rate = EXCLUDED.dmca_rate,
        request_count = EXCLUDED.request_count,
        request_success_count = EXCLUDED.request_success_count,
        acquisition_count = EXCLUDED.acquisition_count,
        acquisition_success_count = EXCLUDED.acquisition_success_count,
        min_samples = EXCLUDED.min_samples,
        computed_at = EXCLUDED.computed_at;
END;
$$;
