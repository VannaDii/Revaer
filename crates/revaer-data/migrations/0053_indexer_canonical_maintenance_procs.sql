-- Canonical maintenance procedures.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'scoring_context') THEN
        CREATE TYPE scoring_context AS ENUM (
            'global_current'
        );
    END IF;
END
$$;

CREATE OR REPLACE FUNCTION canonical_recompute_best_source_v1(
    canonical_torrent_public_id_input UUID,
    scoring_context_input scoring_context DEFAULT 'global_current'
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to recompute best source';
    errcode CONSTANT text := 'P0001';
    canonical_id BIGINT;
    winner_source_id BIGINT;
    winner_source_public_id UUID;
    context_value scoring_context;
BEGIN
    IF canonical_torrent_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'canonical_missing';
    END IF;

    context_value := COALESCE(scoring_context_input, 'global_current');
    IF context_value <> 'global_current' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'scoring_context_invalid';
    END IF;

    SELECT canonical_torrent_id
    INTO canonical_id
    FROM canonical_torrent
    WHERE canonical_torrent_public_id = canonical_torrent_public_id_input;

    IF canonical_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'canonical_not_found';
    END IF;

    SELECT bs.canonical_torrent_source_id,
           src.canonical_torrent_source_public_id
    INTO winner_source_id,
         winner_source_public_id
    FROM canonical_torrent_source_base_score bs
    JOIN canonical_torrent_source src
        ON src.canonical_torrent_source_id = bs.canonical_torrent_source_id
    WHERE bs.canonical_torrent_id = canonical_id
    ORDER BY bs.score_total_base DESC,
             src.last_seen_at DESC,
             src.canonical_torrent_source_public_id ASC
    LIMIT 1;

    IF winner_source_id IS NULL THEN
        SELECT src.canonical_torrent_source_id,
               src.canonical_torrent_source_public_id
        INTO winner_source_id,
             winner_source_public_id
        FROM canonical_torrent_source src
        WHERE EXISTS (
            SELECT 1
            FROM search_request_source_observation obs
            WHERE obs.canonical_torrent_id = canonical_id
              AND obs.canonical_torrent_source_id = src.canonical_torrent_source_id
        )
        ORDER BY COALESCE(src.last_seen_seeders, 0) DESC,
                 src.last_seen_at DESC,
                 src.canonical_torrent_source_public_id ASC
        LIMIT 1;
    END IF;

    IF winner_source_id IS NOT NULL THEN
        INSERT INTO canonical_torrent_best_source_global (
            canonical_torrent_id,
            canonical_torrent_source_id,
            computed_at
        )
        VALUES (
            canonical_id,
            winner_source_id,
            now()
        )
        ON CONFLICT (canonical_torrent_id)
        DO UPDATE SET
            canonical_torrent_source_id = EXCLUDED.canonical_torrent_source_id,
            computed_at = EXCLUDED.computed_at;
    END IF;

    RETURN winner_source_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION canonical_recompute_best_source(
    canonical_torrent_public_id_input UUID,
    scoring_context_input scoring_context DEFAULT 'global_current'
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN canonical_recompute_best_source_v1(
        canonical_torrent_public_id_input,
        scoring_context_input
    );
END;
$$;

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
               magnet_hash,
               title_normalized,
               size_bytes
        FROM canonical_torrent
        WHERE identity_strategy = 'title_size_fallback'
          AND identity_confidence <= 0.60
          AND created_at < cutoff_ts
    ),
    filtered AS (
        SELECT c.*
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
    source_links AS (
        SELECT f.canonical_torrent_id,
               s.canonical_torrent_source_id
        FROM filtered f
        JOIN canonical_torrent_source s
            ON (
                (f.infohash_v2 IS NOT NULL AND s.infohash_v2 = f.infohash_v2)
                OR (
                    f.infohash_v2 IS NULL
                    AND f.infohash_v1 IS NOT NULL
                    AND s.infohash_v2 IS NULL
                    AND s.infohash_v1 = f.infohash_v1
                )
                OR (
                    f.infohash_v2 IS NULL
                    AND f.infohash_v1 IS NULL
                    AND f.magnet_hash IS NOT NULL
                    AND s.infohash_v2 IS NULL
                    AND s.infohash_v1 IS NULL
                    AND s.magnet_hash = f.magnet_hash
                )
                OR (
                    f.infohash_v2 IS NULL
                    AND f.infohash_v1 IS NULL
                    AND f.magnet_hash IS NULL
                    AND s.title_normalized = f.title_normalized
                    AND s.size_bytes = f.size_bytes
                )
            )
    ),
    sources_with_non_pruned AS (
        SELECT DISTINCT sl.canonical_torrent_id
        FROM source_links sl
        JOIN canonical_torrent c
            ON c.canonical_torrent_id <> sl.canonical_torrent_id
        JOIN canonical_torrent_source s
            ON s.canonical_torrent_source_id = sl.canonical_torrent_source_id
        WHERE (
            (c.infohash_v2 IS NOT NULL AND s.infohash_v2 = c.infohash_v2)
            OR (
                c.infohash_v2 IS NULL
                AND c.infohash_v1 IS NOT NULL
                AND s.infohash_v2 IS NULL
                AND s.infohash_v1 = c.infohash_v1
            )
            OR (
                c.infohash_v2 IS NULL
                AND c.infohash_v1 IS NULL
                AND c.magnet_hash IS NOT NULL
                AND s.infohash_v2 IS NULL
                AND s.infohash_v1 IS NULL
                AND s.magnet_hash = c.magnet_hash
            )
            OR (
                c.infohash_v2 IS NULL
                AND c.infohash_v1 IS NULL
                AND c.magnet_hash IS NULL
                AND s.title_normalized = c.title_normalized
                AND s.size_bytes = c.size_bytes
            )
        )
    ),
    eligible AS (
        SELECT f.canonical_torrent_id
        FROM filtered f
        WHERE NOT EXISTS (
            SELECT 1
            FROM sources_with_non_pruned snp
            WHERE snp.canonical_torrent_id = f.canonical_torrent_id
        )
    )
    DELETE FROM canonical_torrent
    WHERE canonical_torrent_id IN (
        SELECT canonical_torrent_id
        FROM eligible
    );
END;
$$;

CREATE OR REPLACE FUNCTION canonical_prune_low_confidence()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM canonical_prune_low_confidence_v1();
END;
$$;
