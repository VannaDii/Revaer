-- Align canonical backfill and base score refresh with durable-source cadence semantics.

CREATE OR REPLACE FUNCTION job_run_canonical_backfill_best_source_v1()
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    cutoff_recent TIMESTAMPTZ;
    canonical_public_id UUID;
BEGIN
    cutoff_recent := now() - make_interval(days => 7);

    FOR canonical_public_id IN
        WITH recent_canonical AS (
            SELECT DISTINCT cs.canonical_torrent_id
            FROM canonical_torrent_source_context_score cs
            JOIN canonical_torrent_source s
                ON s.canonical_torrent_source_id = cs.canonical_torrent_source_id
            WHERE s.last_seen_at >= cutoff_recent
            UNION
            SELECT DISTINCT bs.canonical_torrent_id
            FROM canonical_torrent_source_base_score bs
            JOIN canonical_torrent_source s
                ON s.canonical_torrent_source_id = bs.canonical_torrent_source_id
            WHERE s.last_seen_at >= cutoff_recent
        )
        SELECT c.canonical_torrent_public_id
        FROM canonical_torrent c
        LEFT JOIN canonical_torrent_best_source_global b
            ON b.canonical_torrent_id = c.canonical_torrent_id
        LEFT JOIN recent_canonical r
            ON r.canonical_torrent_id = c.canonical_torrent_id
        WHERE b.canonical_torrent_id IS NULL
           OR r.canonical_torrent_id IS NOT NULL
           OR (c.identity_strategy = 'title_size_fallback' AND c.identity_confidence <= 0.60)
    LOOP
        PERFORM canonical_recompute_best_source_v1(canonical_public_id, 'global_current');
    END LOOP;
END;
$$;

CREATE OR REPLACE FUNCTION job_run_base_score_refresh_recent_v1()
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    cutoff_recent TIMESTAMPTZ;
    canonical_public_id UUID;
BEGIN
    cutoff_recent := now() - make_interval(days => 7);

    WITH candidate_pairs AS (
        SELECT DISTINCT cs.canonical_torrent_id,
                        cs.canonical_torrent_source_id
        FROM canonical_torrent_source_context_score cs
        JOIN canonical_torrent_source s
            ON s.canonical_torrent_source_id = cs.canonical_torrent_source_id
        WHERE s.last_seen_at >= cutoff_recent
        UNION
        SELECT DISTINCT bs.canonical_torrent_id,
                        bs.canonical_torrent_source_id
        FROM canonical_torrent_source_base_score bs
        JOIN canonical_torrent_source s
            ON s.canonical_torrent_source_id = bs.canonical_torrent_source_id
        WHERE s.last_seen_at >= cutoff_recent
    ),
    domain_map AS (
        SELECT imd.indexer_instance_id,
               CASE WHEN COUNT(*) = 1 THEN MAX(md.media_domain_key) ELSE NULL END AS media_domain_key
        FROM indexer_instance_media_domain imd
        JOIN media_domain md
            ON md.media_domain_id = imd.media_domain_id
        GROUP BY imd.indexer_instance_id
    ),
    reputation_latest AS (
        SELECT DISTINCT ON (indexer_instance_id)
               indexer_instance_id,
               request_success_rate,
               acquisition_success_rate,
               request_count,
               acquisition_count
        FROM source_reputation
        WHERE window_key = '24h'
        ORDER BY indexer_instance_id, window_start DESC
    ),
    scored AS (
        SELECT p.canonical_torrent_id,
               s.canonical_torrent_source_id,
               s.last_seen_seeders,
               s.last_seen_leechers,
               s.last_seen_published_at,
               s.last_seen_at,
               COALESCE(t.default_weight, 0) AS score_trust,
               d.media_domain_key,
               cp.status,
               cp.error_class,
               cp.latency_p95_ms,
               COALESCE(r.request_success_rate, 0) AS request_success_rate,
               COALESCE(r.acquisition_success_rate, 0) AS acquisition_success_rate,
               COALESCE(r.request_count, 0) AS request_count,
               COALESCE(r.acquisition_count, 0) AS acquisition_count
        FROM candidate_pairs p
        JOIN canonical_torrent_source s
            ON s.canonical_torrent_source_id = p.canonical_torrent_source_id
        JOIN indexer_instance i
            ON i.indexer_instance_id = s.indexer_instance_id
        LEFT JOIN trust_tier t
            ON t.trust_tier_key = i.trust_tier_key
        LEFT JOIN domain_map d
            ON d.indexer_instance_id = i.indexer_instance_id
        LEFT JOIN indexer_connectivity_profile cp
            ON cp.indexer_instance_id = i.indexer_instance_id
        LEFT JOIN reputation_latest r
            ON r.indexer_instance_id = i.indexer_instance_id
    ),
    weighted AS (
        SELECT s.*,
               CASE s.media_domain_key
                   WHEN 'movies' THEN 1.0
                   WHEN 'tv' THEN 3.0
                   WHEN 'audiobooks' THEN 0.5
                   WHEN 'ebooks' THEN 0.5
                   WHEN 'software' THEN 0.75
                   WHEN 'adult_movies' THEN 1.5
                   WHEN 'adult_scenes' THEN 2.0
                   ELSE 1.0
               END AS media_domain_weight,
               CASE WHEN s.last_seen_published_at IS NULL THEN 0.5 ELSE 1.0 END AS age_multiplier,
               COALESCE(s.last_seen_published_at, s.last_seen_at) AS age_reference
        FROM scored s
    ),
    computed AS (
        SELECT w.canonical_torrent_id,
               w.canonical_torrent_source_id,
               CASE
                   WHEN w.last_seen_seeders IS NULL THEN 0
                   ELSE ln(1 + GREATEST(w.last_seen_seeders, 0)) * 10.0
               END AS score_seed,
               CASE
                   WHEN w.last_seen_leechers IS NULL THEN 0
                   ELSE ln(1 + GREATEST(w.last_seen_leechers, 0)) * 2.0
               END AS score_leech,
               CASE
                   WHEN w.age_reference IS NULL THEN 0
                   WHEN now() - w.age_reference < make_interval(hours => 6)
                   THEN 6 * w.media_domain_weight * w.age_multiplier
                   WHEN now() - w.age_reference < make_interval(hours => 24)
                   THEN 4 * w.media_domain_weight * w.age_multiplier
                   WHEN now() - w.age_reference < make_interval(hours => 72)
                   THEN 2 * w.media_domain_weight * w.age_multiplier
                   WHEN now() - w.age_reference < make_interval(days => 14)
                   THEN 1 * w.media_domain_weight * w.age_multiplier
                   ELSE 0
               END AS score_age,
               w.score_trust,
               CASE
                   WHEN w.status = 'quarantined' THEN
                       -1000
                       + (CASE WHEN w.last_seen_seeders IS NULL THEN -0.5 ELSE 0 END)
                       + (CASE WHEN w.last_seen_leechers IS NULL THEN -0.5 ELSE 0 END)
                   ELSE
                       (CASE
                           WHEN w.latency_p95_ms IS NULL THEN 0
                           WHEN w.latency_p95_ms <= 500 THEN 0
                           WHEN w.latency_p95_ms <= 1500 THEN -2
                           WHEN w.latency_p95_ms <= 4000 THEN -5
                           ELSE -10
                       END)
                       + (CASE
                           WHEN w.error_class = 'http_429' THEN -8
                           WHEN w.error_class = 'timeout' THEN -10
                           WHEN w.error_class = 'cf_challenge' THEN -12
                           WHEN w.error_class = 'auth_error' THEN -15
                           WHEN w.error_class = 'http_403' THEN -10
                           WHEN w.error_class IN ('tls', 'dns', 'connection_refused') THEN -12
                           WHEN w.error_class = 'parse_error' THEN -8
                           WHEN w.error_class = 'http_5xx' THEN -6
                           WHEN w.error_class = 'unknown' THEN -5
                           ELSE 0
                       END)
                       + (CASE WHEN w.last_seen_seeders IS NULL THEN -0.5 ELSE 0 END)
                       + (CASE WHEN w.last_seen_leechers IS NULL THEN -0.5 ELSE 0 END)
               END AS score_health,
               CASE
                   WHEN w.acquisition_count >= 10 THEN
                       GREATEST(LEAST((w.acquisition_success_rate - 0.5) * 10, 5), -5)
                   WHEN w.request_count >= 30 THEN
                       GREATEST(LEAST((w.request_success_rate - 0.5) * 10, 5), -5)
                   ELSE
                       0
               END AS score_reputation
        FROM weighted w
    ),
    totals AS (
        SELECT c.canonical_torrent_id,
               c.canonical_torrent_source_id,
               c.score_seed,
               c.score_leech,
               c.score_age,
               c.score_trust,
               c.score_health,
               c.score_reputation,
               LEAST(10000, GREATEST(-10000,
                   c.score_seed + c.score_leech + c.score_age
                   + c.score_trust + c.score_health + c.score_reputation
               )) AS score_total_base
        FROM computed c
    )
    INSERT INTO canonical_torrent_source_base_score (
        canonical_torrent_id,
        canonical_torrent_source_id,
        score_total_base,
        score_seed,
        score_leech,
        score_age,
        score_trust,
        score_health,
        score_reputation,
        computed_at
    )
    SELECT t.canonical_torrent_id,
           t.canonical_torrent_source_id,
           round(t.score_total_base::NUMERIC, 4),
           t.score_seed,
           t.score_leech,
           t.score_age,
           t.score_trust,
           t.score_health,
           t.score_reputation,
           now()
    FROM totals t
    ON CONFLICT (canonical_torrent_id, canonical_torrent_source_id)
    DO UPDATE SET
        score_total_base = EXCLUDED.score_total_base,
        score_seed = EXCLUDED.score_seed,
        score_leech = EXCLUDED.score_leech,
        score_age = EXCLUDED.score_age,
        score_trust = EXCLUDED.score_trust,
        score_health = EXCLUDED.score_health,
        score_reputation = EXCLUDED.score_reputation,
        computed_at = EXCLUDED.computed_at;

    FOR canonical_public_id IN
        SELECT c.canonical_torrent_public_id
        FROM canonical_torrent c
        WHERE EXISTS (
            SELECT 1
            FROM canonical_torrent_source_context_score cs
            JOIN canonical_torrent_source s
                ON s.canonical_torrent_source_id = cs.canonical_torrent_source_id
            WHERE cs.canonical_torrent_id = c.canonical_torrent_id
              AND s.last_seen_at >= cutoff_recent
            UNION
            SELECT 1
            FROM canonical_torrent_source_base_score bs
            JOIN canonical_torrent_source s
                ON s.canonical_torrent_source_id = bs.canonical_torrent_source_id
            WHERE bs.canonical_torrent_id = c.canonical_torrent_id
              AND s.last_seen_at >= cutoff_recent
        )
    LOOP
        PERFORM canonical_recompute_best_source_v1(canonical_public_id, 'global_current');
    END LOOP;
END;
$$;
