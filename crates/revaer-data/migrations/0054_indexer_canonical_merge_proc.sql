-- Canonical merge by infohash procedure.

CREATE OR REPLACE FUNCTION canonical_merge_by_infohash_v1(
    infohash_v2_input CHAR(64),
    infohash_v1_input CHAR(40)
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to merge canonical';
    errcode CONSTANT text := 'P0001';
    infohash_v2_value TEXT;
    infohash_v1_value TEXT;
    hash_type disambiguation_identity_type;
    hash_value TEXT;
    candidate_count INTEGER;
    winner_id BIGINT;
    winner_public_id UUID;
    winner_strategy identity_strategy;
    winner_infohash_v2 TEXT;
    winner_infohash_v1 TEXT;
    winner_magnet_hash TEXT;
    loser_id BIGINT;
    loser_public_id UUID;
    v2_distinct INTEGER;
    v1_distinct INTEGER;
    magnet_distinct INTEGER;
    v2_value TEXT;
    v1_value TEXT;
    magnet_value TEXT;
    best_imdb_id TEXT;
    best_tmdb_id INTEGER;
    best_tvdb_id INTEGER;
    sample_count INTEGER;
    sample_median NUMERIC(20,4);
    sample_min BIGINT;
    sample_max BIGINT;
    sample_first BIGINT;
    best_title_display TEXT;
BEGIN
    infohash_v2_value := NULLIF(lower(infohash_v2_input), '');
    infohash_v1_value := NULLIF(lower(infohash_v1_input), '');

    IF infohash_v2_value IS NULL AND infohash_v1_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'hash_missing';
    END IF;

    IF infohash_v2_value IS NOT NULL THEN
        IF infohash_v2_value !~ '^[0-9a-f]{64}$' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'hash_invalid';
        END IF;
        hash_type := 'infohash_v2';
        hash_value := infohash_v2_value;
    ELSE
        IF infohash_v1_value !~ '^[0-9a-f]{40}$' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'hash_invalid';
        END IF;
        hash_type := 'infohash_v1';
        hash_value := infohash_v1_value;
    END IF;

    CREATE TEMP TABLE tmp_merge_candidates ON COMMIT DROP AS
    SELECT canonical_torrent_id,
           canonical_torrent_public_id,
           created_at,
           identity_strategy,
           infohash_v2,
           infohash_v1,
           magnet_hash,
           title_normalized,
           size_bytes
    FROM canonical_torrent
    WHERE (infohash_v2_value IS NOT NULL AND infohash_v2 = infohash_v2_value)
       OR (infohash_v2_value IS NULL AND infohash_v1 = infohash_v1_value);

    SELECT COUNT(*)
    INTO candidate_count
    FROM tmp_merge_candidates;

    IF candidate_count < 2 THEN
        RETURN;
    END IF;

    SELECT canonical_torrent_id,
           canonical_torrent_public_id,
           identity_strategy,
           infohash_v2,
           infohash_v1,
           magnet_hash
    INTO winner_id,
         winner_public_id,
         winner_strategy,
         winner_infohash_v2,
         winner_infohash_v1,
         winner_magnet_hash
    FROM tmp_merge_candidates
    ORDER BY created_at ASC, canonical_torrent_id ASC
    LIMIT 1;

    FOR loser_id, loser_public_id IN
        SELECT canonical_torrent_id, canonical_torrent_public_id
        FROM tmp_merge_candidates
        WHERE canonical_torrent_id <> winner_id
    LOOP
        IF EXISTS (
            SELECT 1
            FROM canonical_disambiguation_rule
            WHERE rule_type = 'prevent_merge'
              AND (
                  (
                      identity_left_type = 'canonical_public_id'
                      AND identity_right_type = 'canonical_public_id'
                      AND identity_left_value_uuid = LEAST(winner_public_id, loser_public_id)
                      AND identity_right_value_uuid = GREATEST(winner_public_id, loser_public_id)
                  )
                  OR (
                      identity_left_type = 'canonical_public_id'
                      AND identity_left_value_uuid = winner_public_id
                      AND identity_right_type = hash_type
                      AND identity_right_value_text = hash_value
                  )
                  OR (
                      identity_right_type = 'canonical_public_id'
                      AND identity_right_value_uuid = winner_public_id
                      AND identity_left_type = hash_type
                      AND identity_left_value_text = hash_value
                  )
                  OR (
                      identity_left_type = 'canonical_public_id'
                      AND identity_left_value_uuid = loser_public_id
                      AND identity_right_type = hash_type
                      AND identity_right_value_text = hash_value
                  )
                  OR (
                      identity_right_type = 'canonical_public_id'
                      AND identity_right_value_uuid = loser_public_id
                      AND identity_left_type = hash_type
                      AND identity_left_value_text = hash_value
                  )
              )
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'prevent_merge';
        END IF;
    END LOOP;

    SELECT COUNT(DISTINCT infohash_v2), MIN(infohash_v2)
    INTO v2_distinct, v2_value
    FROM tmp_merge_candidates
    WHERE infohash_v2 IS NOT NULL;

    SELECT COUNT(DISTINCT infohash_v1), MIN(infohash_v1)
    INTO v1_distinct, v1_value
    FROM tmp_merge_candidates
    WHERE infohash_v1 IS NOT NULL;

    SELECT COUNT(DISTINCT magnet_hash), MIN(magnet_hash)
    INTO magnet_distinct, magnet_value
    FROM tmp_merge_candidates
    WHERE magnet_hash IS NOT NULL;

    IF winner_infohash_v2 IS NULL AND v2_distinct = 1 THEN
        UPDATE canonical_torrent
        SET infohash_v2 = v2_value,
            updated_at = now()
        WHERE canonical_torrent_id = winner_id;
        winner_infohash_v2 := v2_value;
    END IF;

    IF winner_infohash_v1 IS NULL AND v1_distinct = 1 THEN
        UPDATE canonical_torrent
        SET infohash_v1 = v1_value,
            updated_at = now()
        WHERE canonical_torrent_id = winner_id;
        winner_infohash_v1 := v1_value;
    END IF;

    IF winner_magnet_hash IS NULL AND magnet_distinct = 1 THEN
        UPDATE canonical_torrent
        SET magnet_hash = magnet_value,
            updated_at = now()
        WHERE canonical_torrent_id = winner_id;
        winner_magnet_hash := magnet_value;
    END IF;

    FOR loser_id, loser_public_id IN
        SELECT canonical_torrent_id, canonical_torrent_public_id
        FROM tmp_merge_candidates
        WHERE canonical_torrent_id <> winner_id
    LOOP
        INSERT INTO canonical_external_id (
            canonical_torrent_id,
            id_type,
            id_value_text,
            trust_tier_rank,
            first_seen_at,
            last_seen_at,
            source_canonical_torrent_source_id
        )
        SELECT winner_id,
               id_type,
               id_value_text,
               trust_tier_rank,
               first_seen_at,
               last_seen_at,
               source_canonical_torrent_source_id
        FROM canonical_external_id
        WHERE canonical_torrent_id = loser_id
          AND id_value_text IS NOT NULL
        ON CONFLICT (canonical_torrent_id, id_type, id_value_text)
        DO UPDATE SET
            first_seen_at = LEAST(canonical_external_id.first_seen_at, EXCLUDED.first_seen_at),
            last_seen_at = GREATEST(canonical_external_id.last_seen_at, EXCLUDED.last_seen_at),
            trust_tier_rank = GREATEST(canonical_external_id.trust_tier_rank, EXCLUDED.trust_tier_rank),
            source_canonical_torrent_source_id = COALESCE(
                canonical_external_id.source_canonical_torrent_source_id,
                EXCLUDED.source_canonical_torrent_source_id
            );

        INSERT INTO canonical_external_id (
            canonical_torrent_id,
            id_type,
            id_value_int,
            trust_tier_rank,
            first_seen_at,
            last_seen_at,
            source_canonical_torrent_source_id
        )
        SELECT winner_id,
               id_type,
               id_value_int,
               trust_tier_rank,
               first_seen_at,
               last_seen_at,
               source_canonical_torrent_source_id
        FROM canonical_external_id
        WHERE canonical_torrent_id = loser_id
          AND id_value_int IS NOT NULL
        ON CONFLICT (canonical_torrent_id, id_type, id_value_int)
        DO UPDATE SET
            first_seen_at = LEAST(canonical_external_id.first_seen_at, EXCLUDED.first_seen_at),
            last_seen_at = GREATEST(canonical_external_id.last_seen_at, EXCLUDED.last_seen_at),
            trust_tier_rank = GREATEST(canonical_external_id.trust_tier_rank, EXCLUDED.trust_tier_rank),
            source_canonical_torrent_source_id = COALESCE(
                canonical_external_id.source_canonical_torrent_source_id,
                EXCLUDED.source_canonical_torrent_source_id
            );

        DELETE FROM canonical_external_id
        WHERE canonical_torrent_id = loser_id;

        INSERT INTO canonical_torrent_signal (
            canonical_torrent_id,
            signal_key,
            value_text,
            value_int,
            confidence,
            parser_version
        )
        SELECT winner_id,
               signal_key,
               value_text,
               value_int,
               confidence,
               parser_version
        FROM canonical_torrent_signal
        WHERE canonical_torrent_id = loser_id
        ON CONFLICT (canonical_torrent_id, signal_key, value_text, value_int)
        DO UPDATE SET
            confidence = LEAST(
                1.0,
                GREATEST(canonical_torrent_signal.confidence, EXCLUDED.confidence) + 0.05
            ),
            parser_version = LEAST(canonical_torrent_signal.parser_version, EXCLUDED.parser_version);

        DELETE FROM canonical_torrent_signal
        WHERE canonical_torrent_id = loser_id;

        DELETE FROM canonical_size_sample loser_sample
        USING canonical_size_sample winner_sample
        WHERE loser_sample.canonical_torrent_id = loser_id
          AND winner_sample.canonical_torrent_id = winner_id
          AND loser_sample.observed_at = winner_sample.observed_at
          AND loser_sample.size_bytes = winner_sample.size_bytes;

        UPDATE canonical_size_sample
        SET canonical_torrent_id = winner_id
        WHERE canonical_torrent_id = loser_id;

        DELETE FROM canonical_size_rollup
        WHERE canonical_torrent_id = loser_id;

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
        SELECT winner_id,
               canonical_torrent_source_id,
               score_total_base,
               score_seed,
               score_leech,
               score_age,
               score_trust,
               score_health,
               score_reputation,
               computed_at
        FROM canonical_torrent_source_base_score
        WHERE canonical_torrent_id = loser_id
        ON CONFLICT (canonical_torrent_id, canonical_torrent_source_id)
        DO UPDATE SET
            score_total_base = CASE
                WHEN EXCLUDED.score_total_base >= canonical_torrent_source_base_score.score_total_base
                    THEN EXCLUDED.score_total_base
                ELSE canonical_torrent_source_base_score.score_total_base
            END,
            score_seed = CASE
                WHEN EXCLUDED.score_total_base >= canonical_torrent_source_base_score.score_total_base
                    THEN EXCLUDED.score_seed
                ELSE canonical_torrent_source_base_score.score_seed
            END,
            score_leech = CASE
                WHEN EXCLUDED.score_total_base >= canonical_torrent_source_base_score.score_total_base
                    THEN EXCLUDED.score_leech
                ELSE canonical_torrent_source_base_score.score_leech
            END,
            score_age = CASE
                WHEN EXCLUDED.score_total_base >= canonical_torrent_source_base_score.score_total_base
                    THEN EXCLUDED.score_age
                ELSE canonical_torrent_source_base_score.score_age
            END,
            score_trust = CASE
                WHEN EXCLUDED.score_total_base >= canonical_torrent_source_base_score.score_total_base
                    THEN EXCLUDED.score_trust
                ELSE canonical_torrent_source_base_score.score_trust
            END,
            score_health = CASE
                WHEN EXCLUDED.score_total_base >= canonical_torrent_source_base_score.score_total_base
                    THEN EXCLUDED.score_health
                ELSE canonical_torrent_source_base_score.score_health
            END,
            score_reputation = CASE
                WHEN EXCLUDED.score_total_base >= canonical_torrent_source_base_score.score_total_base
                    THEN EXCLUDED.score_reputation
                ELSE canonical_torrent_source_base_score.score_reputation
            END,
            computed_at = GREATEST(
                canonical_torrent_source_base_score.computed_at,
                EXCLUDED.computed_at
            );

        DELETE FROM canonical_torrent_source_base_score
        WHERE canonical_torrent_id = loser_id;

        INSERT INTO canonical_torrent_source_context_score (
            context_key_type,
            context_key_id,
            canonical_torrent_id,
            canonical_torrent_source_id,
            score_total_context,
            score_policy_adjust,
            score_tag_adjust,
            is_dropped,
            computed_at
        )
        SELECT context_key_type,
               context_key_id,
               winner_id,
               canonical_torrent_source_id,
               score_total_context,
               score_policy_adjust,
               score_tag_adjust,
               is_dropped,
               computed_at
        FROM canonical_torrent_source_context_score
        WHERE canonical_torrent_id = loser_id
        ON CONFLICT (context_key_type, context_key_id, canonical_torrent_id, canonical_torrent_source_id)
        DO UPDATE SET
            score_total_context = CASE
                WHEN EXCLUDED.score_total_context >= canonical_torrent_source_context_score.score_total_context
                    THEN EXCLUDED.score_total_context
                ELSE canonical_torrent_source_context_score.score_total_context
            END,
            score_policy_adjust = CASE
                WHEN EXCLUDED.score_total_context >= canonical_torrent_source_context_score.score_total_context
                    THEN EXCLUDED.score_policy_adjust
                ELSE canonical_torrent_source_context_score.score_policy_adjust
            END,
            score_tag_adjust = CASE
                WHEN EXCLUDED.score_total_context >= canonical_torrent_source_context_score.score_total_context
                    THEN EXCLUDED.score_tag_adjust
                ELSE canonical_torrent_source_context_score.score_tag_adjust
            END,
            is_dropped = CASE
                WHEN EXCLUDED.score_total_context >= canonical_torrent_source_context_score.score_total_context
                    THEN EXCLUDED.is_dropped
                ELSE canonical_torrent_source_context_score.is_dropped
            END,
            computed_at = GREATEST(
                canonical_torrent_source_context_score.computed_at,
                EXCLUDED.computed_at
            );

        DELETE FROM canonical_torrent_source_context_score
        WHERE canonical_torrent_id = loser_id;

        DELETE FROM canonical_torrent_best_source_global
        WHERE canonical_torrent_id = loser_id;

        DELETE FROM canonical_torrent_best_source_context loser_context
        USING canonical_torrent_best_source_context winner_context
        WHERE loser_context.canonical_torrent_id = loser_id
          AND winner_context.canonical_torrent_id = winner_id
          AND loser_context.context_key_type = winner_context.context_key_type
          AND loser_context.context_key_id = winner_context.context_key_id;

        UPDATE canonical_torrent_best_source_context
        SET canonical_torrent_id = winner_id
        WHERE canonical_torrent_id = loser_id;

        DELETE FROM search_request_canonical loser_link
        USING search_request_canonical winner_link
        WHERE loser_link.canonical_torrent_id = loser_id
          AND winner_link.canonical_torrent_id = winner_id
          AND loser_link.search_request_id = winner_link.search_request_id;

        UPDATE search_request_canonical
        SET canonical_torrent_id = winner_id
        WHERE canonical_torrent_id = loser_id;

        UPDATE search_request_source_observation
        SET canonical_torrent_id = winner_id
        WHERE canonical_torrent_id = loser_id;

        UPDATE search_filter_decision
        SET canonical_torrent_id = winner_id
        WHERE canonical_torrent_id = loser_id;

        UPDATE user_result_action
        SET canonical_torrent_id = winner_id
        WHERE canonical_torrent_id = loser_id;

        UPDATE acquisition_attempt
        SET canonical_torrent_id = winner_id
        WHERE canonical_torrent_id = loser_id;

        DELETE FROM canonical_torrent
        WHERE canonical_torrent_id = loser_id;
    END LOOP;

    SELECT COUNT(*), percentile_cont(0.5) WITHIN GROUP (ORDER BY size_bytes),
           MIN(size_bytes), MAX(size_bytes)
    INTO sample_count, sample_median, sample_min, sample_max
    FROM canonical_size_sample
    WHERE canonical_torrent_id = winner_id;

    IF sample_count IS NOT NULL AND sample_count > 0 THEN
        SELECT size_bytes
        INTO sample_first
        FROM canonical_size_sample
        WHERE canonical_torrent_id = winner_id
        ORDER BY observed_at ASC, canonical_size_sample_id ASC
        LIMIT 1;

        INSERT INTO canonical_size_rollup (
            canonical_torrent_id,
            sample_count,
            size_median,
            size_min,
            size_max,
            updated_at
        )
        VALUES (
            winner_id,
            sample_count,
            sample_median::BIGINT,
            sample_min,
            sample_max,
            now()
        )
        ON CONFLICT (canonical_torrent_id)
        DO UPDATE SET
            sample_count = EXCLUDED.sample_count,
            size_median = EXCLUDED.size_median,
            size_min = EXCLUDED.size_min,
            size_max = EXCLUDED.size_max,
            updated_at = EXCLUDED.updated_at;

        IF winner_strategy <> 'title_size_fallback' THEN
            UPDATE canonical_torrent
            SET size_bytes = CASE
                    WHEN sample_count >= 3 THEN sample_median::BIGINT
                    ELSE COALESCE(sample_first, sample_min)
                END,
                updated_at = now()
            WHERE canonical_torrent_id = winner_id;
        END IF;
    ELSE
        DELETE FROM canonical_size_rollup
        WHERE canonical_torrent_id = winner_id;
    END IF;

    SELECT id_value_text
    INTO best_imdb_id
    FROM canonical_external_id
    WHERE canonical_torrent_id = winner_id
      AND id_type = 'imdb'
    ORDER BY trust_tier_rank DESC,
             last_seen_at DESC,
             canonical_external_id_id ASC
    LIMIT 1;

    IF best_imdb_id IS NOT NULL THEN
        UPDATE canonical_torrent
        SET imdb_id = best_imdb_id,
            updated_at = now()
        WHERE canonical_torrent_id = winner_id;
    END IF;

    SELECT id_value_int
    INTO best_tmdb_id
    FROM canonical_external_id
    WHERE canonical_torrent_id = winner_id
      AND id_type = 'tmdb'
    ORDER BY trust_tier_rank DESC,
             last_seen_at DESC,
             canonical_external_id_id ASC
    LIMIT 1;

    IF best_tmdb_id IS NOT NULL THEN
        UPDATE canonical_torrent
        SET tmdb_id = best_tmdb_id,
            updated_at = now()
        WHERE canonical_torrent_id = winner_id;
    END IF;

    SELECT id_value_int
    INTO best_tvdb_id
    FROM canonical_external_id
    WHERE canonical_torrent_id = winner_id
      AND id_type = 'tvdb'
    ORDER BY trust_tier_rank DESC,
             last_seen_at DESC,
             canonical_external_id_id ASC
    LIMIT 1;

    IF best_tvdb_id IS NOT NULL THEN
        UPDATE canonical_torrent
        SET tvdb_id = best_tvdb_id,
            updated_at = now()
        WHERE canonical_torrent_id = winner_id;
    END IF;

    SELECT obs.title_raw
    INTO best_title_display
    FROM search_request_source_observation obs
    JOIN indexer_instance inst
        ON inst.indexer_instance_id = obs.indexer_instance_id
    LEFT JOIN trust_tier tt
        ON tt.trust_tier_key = inst.trust_tier_key
    LEFT JOIN canonical_torrent_source_base_score bs
        ON bs.canonical_torrent_id = winner_id
       AND bs.canonical_torrent_source_id = obs.canonical_torrent_source_id
    WHERE obs.canonical_torrent_id = winner_id
    ORDER BY COALESCE(tt.rank, 0) DESC,
             COALESCE(bs.score_total_base, 0) DESC,
             obs.observed_at DESC,
             obs.observation_id ASC
    LIMIT 1;

    IF best_title_display IS NOT NULL THEN
        UPDATE canonical_torrent
        SET title_display = best_title_display,
            updated_at = now()
        WHERE canonical_torrent_id = winner_id;
    END IF;

    PERFORM canonical_recompute_best_source_v1(
        (SELECT canonical_torrent_public_id FROM canonical_torrent WHERE canonical_torrent_id = winner_id),
        'global_current'
    );
END;
$$;

CREATE OR REPLACE FUNCTION canonical_merge_by_infohash(
    infohash_v2_input CHAR(64),
    infohash_v1_input CHAR(40)
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM canonical_merge_by_infohash_v1(infohash_v2_input, infohash_v1_input);
END;
$$;
