INSERT INTO import_indexer_result (
    import_job_id,
    prowlarr_identifier,
    upstream_slug,
    indexer_instance_id,
    status,
    detail,
    resolved_is_enabled,
    resolved_priority,
    missing_secret_fields
)
VALUES (
    $1,
    'prowlarr-snapshot',
    $2,
    NULL,
    'imported_needs_secret',
    'missing_secret_bindings',
    FALSE,
    73,
    2
)
RETURNING import_indexer_result_id
