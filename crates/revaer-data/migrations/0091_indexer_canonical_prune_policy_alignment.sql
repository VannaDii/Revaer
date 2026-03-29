-- Align low-confidence canonical pruning with explicit durable-source linkage rules.

CREATE OR REPLACE FUNCTION canonical_prune_low_confidence_v1()
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    cutoff_ts TIMESTAMPTZ;
BEGIN
    cutoff_ts := now() - make_interval(days => 30);

    WITH candidates AS (
        SELECT canonical_torrent_id,
               infohash_v1,
               infohash_v2,
               magnet_hash
        FROM canonical_torrent
        WHERE identity_strategy = 'title_size_fallback'
          AND identity_confidence <= 0.60
          AND created_at < cutoff_ts
    ),
    filtered AS (
        SELECT c.canonical_torrent_id,
               c.infohash_v1,
               c.infohash_v2,
               c.magnet_hash
        FROM candidates c
        WHERE NOT EXISTS (
            SELECT 1
            FROM acquisition_attempt a
            WHERE a.canonical_torrent_id = c.canonical_torrent_id
               OR (c.infohash_v1 IS NOT NULL AND a.infohash_v1 = c.infohash_v1)
               OR (c.infohash_v2 IS NOT NULL AND a.infohash_v2 = c.infohash_v2)
               OR (c.magnet_hash IS NOT NULL AND a.magnet_hash = c.magnet_hash)
        )
          AND NOT EXISTS (
            SELECT 1
            FROM user_result_action ura
            WHERE ura.canonical_torrent_id = c.canonical_torrent_id
              AND ura.action IN ('selected', 'downloaded')
        )
    ),
    canonical_source_link AS (
        SELECT canonical_torrent_id,
               canonical_torrent_source_id
        FROM canonical_torrent_source_base_score
        UNION
        SELECT canonical_torrent_id,
               canonical_torrent_source_id
        FROM canonical_torrent_source_context_score
        UNION
        SELECT canonical_torrent_id,
               canonical_torrent_source_id
        FROM canonical_torrent_best_source_global
        UNION
        SELECT canonical_torrent_id,
               canonical_torrent_source_id
        FROM canonical_torrent_best_source_context
    ),
    candidate_sources AS (
        SELECT link.canonical_torrent_id,
               link.canonical_torrent_source_id
        FROM canonical_source_link link
        JOIN filtered candidate
            ON candidate.canonical_torrent_id = link.canonical_torrent_id
    ),
    sources_with_non_candidate_links AS (
        SELECT DISTINCT source_link.canonical_torrent_id
        FROM candidate_sources source_link
        JOIN canonical_source_link link
            ON link.canonical_torrent_source_id = source_link.canonical_torrent_source_id
           AND link.canonical_torrent_id <> source_link.canonical_torrent_id
        LEFT JOIN filtered candidate
            ON candidate.canonical_torrent_id = link.canonical_torrent_id
        WHERE candidate.canonical_torrent_id IS NULL
    ),
    eligible AS (
        SELECT candidate.canonical_torrent_id
        FROM filtered candidate
        WHERE NOT EXISTS (
            SELECT 1
            FROM sources_with_non_candidate_links blocked
            WHERE blocked.canonical_torrent_id = candidate.canonical_torrent_id
        )
    )
    DELETE FROM canonical_torrent
    WHERE canonical_torrent_id IN (
        SELECT canonical_torrent_id
        FROM eligible
    );
END;
$$;
