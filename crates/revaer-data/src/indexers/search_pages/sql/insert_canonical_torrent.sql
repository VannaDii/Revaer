INSERT INTO canonical_torrent (
    canonical_torrent_public_id,
    identity_confidence,
    identity_strategy,
    infohash_v1,
    title_display,
    title_normalized,
    size_bytes
)
VALUES ($1, $2, $3::identity_strategy, $4, $5, $6, $7)
RETURNING canonical_torrent_id
