-- Align connectivity rollups and status thresholds with ERD rules.

CREATE OR REPLACE FUNCTION job_run_connectivity_profile_refresh_v1()
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    now_ts TIMESTAMPTZ;
BEGIN
    now_ts := now();

    WITH indexer_scope AS (
        SELECT indexer_instance_id
        FROM indexer_instance
        WHERE deleted_at IS NULL
    ),
    samples_1h AS (
        SELECT indexer_instance_id,
               COUNT(*) AS total_samples,
               COUNT(*) FILTER (WHERE outcome = 'success' AND parse_ok = TRUE) AS success_count,
               COUNT(*) FILTER (WHERE outcome = 'failure') AS failure_count,
               COUNT(*) FILTER (WHERE error_class = 'http_429') AS http_429_count,
               COUNT(latency_ms) FILTER (WHERE latency_ms IS NOT NULL) AS latency_count,
               percentile_cont(0.5) WITHIN GROUP (ORDER BY latency_ms) AS latency_p50,
               percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms) AS latency_p95
        FROM outbound_request_log
        WHERE request_type IN ('caps', 'search', 'tvsearch', 'moviesearch', 'rss', 'probe')
          AND finished_at >= now_ts - make_interval(hours => 1)
          AND error_class IS DISTINCT FROM 'rate_limited'
        GROUP BY indexer_instance_id
    ),
    samples_24h AS (
        SELECT indexer_instance_id,
               COUNT(*) AS total_samples,
               COUNT(*) FILTER (WHERE outcome = 'success' AND parse_ok = TRUE) AS success_count
        FROM outbound_request_log
        WHERE request_type IN ('caps', 'search', 'tvsearch', 'moviesearch', 'rss', 'probe')
          AND finished_at >= now_ts - make_interval(hours => 24)
          AND error_class IS DISTINCT FROM 'rate_limited'
        GROUP BY indexer_instance_id
    ),
    failures_1h AS (
        SELECT indexer_instance_id,
               error_class,
               COUNT(*) AS failure_count
        FROM outbound_request_log
        WHERE request_type IN ('caps', 'search', 'tvsearch', 'moviesearch', 'rss', 'probe')
          AND finished_at >= now_ts - make_interval(hours => 1)
          AND outcome = 'failure'
          AND error_class IS DISTINCT FROM 'rate_limited'
        GROUP BY indexer_instance_id, error_class
    ),
    dominant_failures AS (
        SELECT DISTINCT ON (indexer_instance_id)
               indexer_instance_id,
               error_class,
               failure_count
        FROM failures_1h
        ORDER BY indexer_instance_id, failure_count DESC, error_class ASC
    ),
    burst_10m AS (
        SELECT indexer_instance_id,
               COUNT(*) AS total_samples,
               COUNT(*) FILTER (WHERE error_class = 'http_429') AS http_429_count
        FROM outbound_request_log
        WHERE request_type IN ('caps', 'search', 'tvsearch', 'moviesearch', 'rss', 'probe')
          AND finished_at >= now_ts - make_interval(mins => 10)
          AND error_class IS DISTINCT FROM 'rate_limited'
        GROUP BY indexer_instance_id
    ),
    combined AS (
        SELECT s.indexer_instance_id,
               s1.total_samples AS total_samples_1h,
               s1.success_count AS success_count_1h,
               s1.failure_count AS failure_count_1h,
               s1.http_429_count AS http_429_count_1h,
               s1.latency_count,
               s1.latency_p50,
               s1.latency_p95,
               s24.total_samples AS total_samples_24h,
               s24.success_count AS success_count_24h,
               df.error_class AS dominant_error_class,
               df.failure_count AS dominant_failure_count,
               b10.total_samples AS total_samples_10m,
               b10.http_429_count AS http_429_count_10m
        FROM indexer_scope s
        LEFT JOIN samples_1h s1
            ON s1.indexer_instance_id = s.indexer_instance_id
        LEFT JOIN samples_24h s24
            ON s24.indexer_instance_id = s.indexer_instance_id
        LEFT JOIN dominant_failures df
            ON df.indexer_instance_id = s.indexer_instance_id
        LEFT JOIN burst_10m b10
            ON b10.indexer_instance_id = s.indexer_instance_id
    ),
    derived AS (
        SELECT c.indexer_instance_id,
               CASE
                   WHEN c.total_samples_1h IS NULL OR c.total_samples_1h = 0 THEN NULL
                   ELSE c.success_count_1h::NUMERIC / c.total_samples_1h
               END AS success_rate_1h,
               CASE
                   WHEN c.total_samples_24h IS NULL OR c.total_samples_24h = 0 THEN NULL
                   ELSE c.success_count_24h::NUMERIC / c.total_samples_24h
               END AS success_rate_24h,
               CASE
                   WHEN c.latency_count IS NULL OR c.latency_count < 5 THEN 0
                   WHEN c.latency_count < 20 THEN COALESCE(c.latency_p50, 0)
                   ELSE COALESCE(c.latency_p95, 0)
               END AS effective_latency_ms,
               c.latency_p50,
               c.latency_p95,
               CASE
                   WHEN c.dominant_failure_count IS NOT NULL
                        AND c.total_samples_1h IS NOT NULL
                        AND c.dominant_failure_count >= 5
                        AND c.dominant_failure_count >= (c.total_samples_1h * 0.2)
                   THEN c.dominant_error_class
                   ELSE NULL
               END AS dominant_error_class,
               CASE
                   WHEN c.http_429_count_10m IS NOT NULL
                        AND c.http_429_count_10m >= 10 THEN TRUE
                   WHEN c.http_429_count_10m IS NOT NULL
                        AND c.total_samples_10m IS NOT NULL
                        AND c.total_samples_10m >= 20
                        AND (c.http_429_count_10m::NUMERIC / c.total_samples_10m) >= 0.3
                   THEN TRUE
                   ELSE FALSE
               END AS http_429_burst
        FROM combined c
    ),
    scored AS (
        SELECT d.indexer_instance_id,
               d.success_rate_1h,
               d.success_rate_24h,
               d.latency_p50,
               d.latency_p95,
               d.dominant_error_class,
               d.http_429_burst,
               CASE
                   WHEN d.success_rate_1h IS NULL THEN 'degraded'::connectivity_status
                   WHEN d.success_rate_1h >= 0.98 AND d.effective_latency_ms <= 1500 THEN 'healthy'::connectivity_status
                   WHEN d.success_rate_1h < 0.90 THEN 'failing'::connectivity_status
                   WHEN d.dominant_error_class IN ('auth_error', 'cf_challenge', 'tls', 'dns') THEN 'failing'::connectivity_status
                   WHEN d.success_rate_1h >= 0.90 OR d.effective_latency_ms <= 4000 THEN 'degraded'::connectivity_status
                   ELSE 'failing'::connectivity_status
               END AS base_status
        FROM derived d
    ),
    with_prev AS (
        SELECT s.indexer_instance_id,
               s.success_rate_1h,
               s.success_rate_24h,
               s.latency_p50,
               s.latency_p95,
               s.dominant_error_class,
               s.http_429_burst,
               s.base_status,
               p.status AS prev_status,
               p.error_class AS prev_error_class,
               p.last_checked_at AS prev_checked_at
        FROM scored s
        LEFT JOIN indexer_connectivity_profile p
            ON p.indexer_instance_id = s.indexer_instance_id
    ),
    status_resolved AS (
        SELECT w.indexer_instance_id,
               CASE
                   WHEN w.prev_status = 'quarantined'
                        AND w.prev_checked_at >= now_ts - make_interval(mins => 30)
                   THEN 'quarantined'::connectivity_status
                   WHEN w.prev_status = 'quarantined'
                        AND w.base_status = 'healthy'
                   THEN 'degraded'::connectivity_status
                   WHEN w.base_status = 'failing'
                        AND (w.dominant_error_class IN ('cf_challenge', 'auth_error') OR w.http_429_burst)
                        AND w.prev_status IN ('failing', 'quarantined')
                        AND w.prev_checked_at <= now_ts - make_interval(mins => 30)
                   THEN 'quarantined'::connectivity_status
                   ELSE w.base_status
               END AS status,
               w.dominant_error_class,
               w.http_429_burst,
               w.prev_error_class,
               w.success_rate_1h,
               w.success_rate_24h,
               w.latency_p50,
               w.latency_p95
        FROM with_prev w
    ),
    final AS (
        SELECT s.indexer_instance_id,
               s.status,
               CASE
                   WHEN s.status = 'healthy' THEN NULL
                   WHEN s.dominant_error_class IS NOT NULL THEN s.dominant_error_class
                   WHEN s.http_429_burst THEN 'http_429'::error_class
                   WHEN s.prev_error_class IS NOT NULL THEN s.prev_error_class
                   ELSE 'unknown'::error_class
               END AS error_class,
               s.success_rate_1h,
               s.success_rate_24h,
               s.latency_p50,
               s.latency_p95
        FROM status_resolved s
    )
    INSERT INTO indexer_connectivity_profile (
        indexer_instance_id,
        status,
        error_class,
        latency_p50_ms,
        latency_p95_ms,
        success_rate_1h,
        success_rate_24h,
        last_checked_at
    )
    SELECT f.indexer_instance_id,
           f.status,
           CASE WHEN f.status = 'healthy' THEN NULL ELSE f.error_class END,
           CASE WHEN f.latency_p50 IS NULL THEN NULL ELSE f.latency_p50::INTEGER END,
           CASE WHEN f.latency_p95 IS NULL THEN NULL ELSE f.latency_p95::INTEGER END,
           f.success_rate_1h,
           f.success_rate_24h,
           now_ts
    FROM final f
    ON CONFLICT (indexer_instance_id)
    DO UPDATE SET
        status = EXCLUDED.status,
        error_class = EXCLUDED.error_class,
        latency_p50_ms = EXCLUDED.latency_p50_ms,
        latency_p95_ms = EXCLUDED.latency_p95_ms,
        success_rate_1h = EXCLUDED.success_rate_1h,
        success_rate_24h = EXCLUDED.success_rate_24h,
        last_checked_at = EXCLUDED.last_checked_at;
END;
$$;
