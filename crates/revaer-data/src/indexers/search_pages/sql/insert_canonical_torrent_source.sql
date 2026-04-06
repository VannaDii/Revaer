INSERT INTO canonical_torrent_source (
    indexer_instance_id,
    canonical_torrent_source_public_id,
    source_guid,
    infohash_v1,
    title_normalized,
    size_bytes
)
VALUES ($1, $2, $3, $4, $5, $6)
RETURNING canonical_torrent_source_id
