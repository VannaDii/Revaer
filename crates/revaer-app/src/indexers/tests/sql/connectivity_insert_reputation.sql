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
VALUES (
    $1,
    '24h'::reputation_window,
    $2 - interval '24 hours',
    0.9500,
    0.9000,
    0.0200,
    0.0100,
    40,
    38,
    10,
    9,
    5,
    $2
)
