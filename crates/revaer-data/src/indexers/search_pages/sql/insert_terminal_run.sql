INSERT INTO search_request_indexer_run (
    search_request_id,
    indexer_instance_id,
    status,
    started_at,
    finished_at,
    error_class
)
VALUES ($1, $2, $3::run_status, now() - make_interval(mins => 1), now(), $4::error_class)
