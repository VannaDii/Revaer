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
VALUES ($1, 'healthy'::connectivity_status, NULL, 110, 240, 0.9700, 0.9500, $2)
