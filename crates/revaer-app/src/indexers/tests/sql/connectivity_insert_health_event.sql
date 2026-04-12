INSERT INTO indexer_health_event (
    indexer_instance_id,
    occurred_at,
    event_type,
    latency_ms,
    http_status,
    error_class,
    detail
)
VALUES (
    $1,
    $2,
    'identity_conflict'::health_event_type,
    503,
    429,
    'cf_challenge'::error_class,
    'challenge observed'
)
