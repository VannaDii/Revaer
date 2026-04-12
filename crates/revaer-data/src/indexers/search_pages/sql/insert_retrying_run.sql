INSERT INTO search_request_indexer_run (
    search_request_id,
    indexer_instance_id,
    status,
    next_attempt_at,
    last_error_class,
    last_rate_limit_scope,
    error_class
)
VALUES ($1, $2, $3::run_status, $4, $5::error_class, $6::rate_limit_scope, $7::error_class)
