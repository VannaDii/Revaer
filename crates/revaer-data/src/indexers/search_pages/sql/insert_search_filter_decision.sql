INSERT INTO search_filter_decision (
    search_request_id,
    policy_rule_public_id,
    policy_snapshot_id,
    canonical_torrent_id,
    canonical_torrent_source_id,
    decision,
    decided_at
)
VALUES ($1, $2, $3, $4, $5, $6::decision_type, now())
