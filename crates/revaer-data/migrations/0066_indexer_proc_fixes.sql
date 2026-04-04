-- Procedure fixes for normalization, import job status, and base score refresh mapping.

CREATE OR REPLACE FUNCTION normalize_title_v1(title_raw TEXT)
RETURNS TEXT
LANGUAGE plpgsql
AS $$
DECLARE
    value TEXT;
    token TEXT;
    tokens TEXT[] := ARRAY[
        '2160p', '1080p', '720p', '480p', '4320p', '4k', '8k',
        'web', 'webrip', 'web dl', 'webdl', 'bluray', 'blu ray', 'bdrip', 'brip',
        'dvdrip', 'hdrip', 'hdtv', 'tvrip', 'cam', 'ts', 'tc', 'scr', 'screener',
        'x264', 'x265', 'h264', 'h265', 'hevc', 'avc', 'xvid', 'divx', 'vp9', 'av1',
        'hdr', 'hdr10', 'hdr10plus', 'dv', 'dolbyvision',
        'aac', 'ac3', 'eac3', 'ddp', 'dts', 'dtshd', 'truehd', 'atmos', 'flac', 'mp3', 'opus',
        'mkv', 'mp4', 'avi',
        'repack', 'proper', 'rerip', 'extended', 'uncut', 'remux',
        'multi', 'dual', 'eng', 'en', 'ita', 'it', 'spa', 'es', 'fre', 'fr', 'ger', 'de', 'jpn',
        'jp', 'kor', 'kr'
    ];
BEGIN
    IF title_raw IS NULL THEN
        RETURN NULL;
    END IF;

    value := lower(unaccent(title_raw));
    value := regexp_replace(value, E'\\b(2\\.0|5\\.1|7\\.1)\\b', ' ', 'g');
    value := regexp_replace(value, '[^a-z0-9]+', ' ', 'g');

    FOREACH token IN ARRAY tokens LOOP
        value := regexp_replace(
            value,
            E'\\m' || replace(token, ' ', E'\\s+') || E'\\M',
            ' ',
            'g'
        );
    END LOOP;

    value := regexp_replace(value, E'\\s+', ' ', 'g');
    value := btrim(value);

    RETURN value;
END;
$$;

CREATE OR REPLACE FUNCTION import_job_get_status_v1(
    import_job_public_id_input UUID
)
RETURNS TABLE(
    status import_job_status,
    result_total INTEGER,
    result_imported_ready INTEGER,
    result_imported_needs_secret INTEGER,
    result_imported_test_failed INTEGER,
    result_unmapped_definition INTEGER,
    result_skipped_duplicate INTEGER
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to fetch import status';
    errcode CONSTANT text := 'P0001';
    job_id BIGINT;
BEGIN
    IF import_job_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'import_job_missing';
    END IF;

    SELECT import_job_id, import_job.status
    INTO job_id, status
    FROM import_job
    WHERE import_job_public_id = import_job_public_id_input;

    IF job_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'import_job_not_found';
    END IF;

    SELECT COUNT(*),
           COUNT(*) FILTER (WHERE import_indexer_result.status = 'imported_ready'),
           COUNT(*) FILTER (WHERE import_indexer_result.status = 'imported_needs_secret'),
           COUNT(*) FILTER (WHERE import_indexer_result.status = 'imported_test_failed'),
           COUNT(*) FILTER (WHERE import_indexer_result.status = 'unmapped_definition'),
           COUNT(*) FILTER (WHERE import_indexer_result.status = 'skipped_duplicate')
    INTO result_total,
         result_imported_ready,
         result_imported_needs_secret,
         result_imported_test_failed,
         result_unmapped_definition,
         result_skipped_duplicate
    FROM import_indexer_result
    WHERE import_job_id = job_id;

    RETURN NEXT;
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
        )
    LOOP
        PERFORM canonical_recompute_best_source_v1(canonical_public_id, 'global_current');
    END LOOP;
END;
$$;
CREATE OR REPLACE FUNCTION rss_poll_apply_v1(
    rss_subscription_id_input BIGINT,
    correlation_id_input UUID,
    retry_seq_input SMALLINT,
    started_at_input TIMESTAMPTZ,
    finished_at_input TIMESTAMPTZ,
    outcome_input outbound_request_outcome,
    error_class_input error_class,
    http_status_input INTEGER,
    latency_ms_input INTEGER,
    parse_ok_input BOOLEAN,
    result_count_input INTEGER,
    via_mitigation_input outbound_via_mitigation,
    rate_limit_denied_scope_input rate_limit_scope,
    cf_detected_input BOOLEAN,
    cf_retryable_input BOOLEAN,
    item_guid_input VARCHAR[],
    infohash_v1_input CHAR(40)[],
    infohash_v2_input CHAR(64)[],
    magnet_hash_input CHAR(64)[]
)
RETURNS TABLE (
    items_parsed INTEGER,
    items_eligible INTEGER,
    items_inserted INTEGER,
    subscription_succeeded BOOLEAN
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to apply RSS poll';
    errcode CONSTANT text := 'P0001';
    subscription_id_value BIGINT;
    instance_id_value BIGINT;
    instance_public_id_value UUID;
    routing_policy_public_id_value UUID;
    interval_seconds_value INTEGER;
    current_backoff_seconds INTEGER;
    now_value TIMESTAMPTZ := now();
    effective_outcome outbound_request_outcome;
    effective_error_class error_class;
    parsed_success BOOLEAN;
    retryable_failure BOOLEAN;
    new_backoff_seconds INTEGER;
    jitter_pct INTEGER;
    jitter_seconds INTEGER;
    max_len INTEGER;
    detail_summary TEXT;
    cf_state_value cf_state;
    cf_consecutive_failures INTEGER;
    cf_backoff_seconds INTEGER;
BEGIN
    IF rss_subscription_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'rss_subscription_missing';
    END IF;

    IF correlation_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'correlation_id_missing';
    END IF;

    IF retry_seq_input IS NULL OR retry_seq_input < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'retry_seq_invalid';
    END IF;

    IF started_at_input IS NULL OR finished_at_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'timestamp_missing';
    END IF;

    IF outcome_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'outcome_missing';
    END IF;

    IF parse_ok_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'parse_ok_missing';
    END IF;

    IF via_mitigation_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'via_mitigation_missing';
    END IF;

    IF cf_detected_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'cf_detected_missing';
    END IF;

    IF cf_retryable_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'cf_retryable_missing';
    END IF;

    IF latency_ms_input IS NOT NULL AND latency_ms_input < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'latency_invalid';
    END IF;

    IF result_count_input IS NOT NULL AND result_count_input < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'result_count_invalid';
    END IF;

    SELECT
        sub.indexer_rss_subscription_id,
        sub.indexer_instance_id,
        sub.interval_seconds,
        sub.backoff_seconds,
        inst.indexer_instance_public_id,
        rp.routing_policy_public_id
    INTO
        subscription_id_value,
        instance_id_value,
        interval_seconds_value,
        current_backoff_seconds,
        instance_public_id_value,
        routing_policy_public_id_value
    FROM indexer_rss_subscription sub
    JOIN indexer_instance inst
        ON inst.indexer_instance_id = sub.indexer_instance_id
    LEFT JOIN routing_policy rp
        ON rp.routing_policy_id = inst.routing_policy_id
    WHERE sub.indexer_rss_subscription_id = rss_subscription_id_input
    FOR UPDATE OF sub;

    IF subscription_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'rss_subscription_not_found';
    END IF;

    effective_outcome := outcome_input;
    effective_error_class := error_class_input;

    IF outcome_input = 'success' THEN
        IF parse_ok_input IS DISTINCT FROM TRUE OR result_count_input IS NULL THEN
            effective_outcome := 'failure';
            effective_error_class := 'parse_error';
        ELSE
            IF error_class_input IS NOT NULL THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'error_class_not_allowed';
            END IF;
        END IF;
    ELSE
        IF error_class_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'error_class_missing';
        END IF;
    END IF;

    parsed_success := (
        effective_outcome = 'success'
        AND parse_ok_input IS TRUE
        AND result_count_input IS NOT NULL
    );

    items_parsed := COALESCE(result_count_input, 0);
    items_eligible := 0;
    items_inserted := 0;

    IF parsed_success THEN
        max_len := GREATEST(
            COALESCE(array_length(item_guid_input, 1), 0),
            COALESCE(array_length(infohash_v1_input, 1), 0),
            COALESCE(array_length(infohash_v2_input, 1), 0),
            COALESCE(array_length(magnet_hash_input, 1), 0)
        );

        IF max_len > 0 THEN
            WITH item_rows AS (
                SELECT
                    i,
                    NULLIF(btrim(item_guid_input[i]), '') AS item_guid_raw,
                    lower(btrim(infohash_v1_input[i])) AS infohash_v1_raw,
                    lower(btrim(infohash_v2_input[i])) AS infohash_v2_raw,
                    lower(btrim(magnet_hash_input[i])) AS magnet_hash_raw
                FROM generate_series(1, max_len) AS i
            ),
            normalized AS (
                SELECT
                    item_guid_raw AS item_guid,
                    CASE
                        WHEN infohash_v1_raw ~ '^[0-9a-f]{40}$' THEN infohash_v1_raw
                        ELSE NULL
                    END AS infohash_v1,
                    CASE
                        WHEN infohash_v2_raw ~ '^[0-9a-f]{64}$' THEN infohash_v2_raw
                        ELSE NULL
                    END AS infohash_v2,
                    CASE
                        WHEN magnet_hash_raw ~ '^[0-9a-f]{64}$' THEN magnet_hash_raw
                        ELSE NULL
                    END AS magnet_hash
                FROM item_rows
            ),
            eligible AS (
                SELECT *
                FROM normalized
                WHERE item_guid IS NOT NULL
                   OR infohash_v1 IS NOT NULL
                   OR infohash_v2 IS NOT NULL
                   OR magnet_hash IS NOT NULL
            ),
            inserted AS (
                INSERT INTO indexer_rss_item_seen (
                    indexer_instance_id,
                    item_guid,
                    infohash_v1,
                    infohash_v2,
                    magnet_hash,
                    first_seen_at
                )
                SELECT
                    instance_id_value,
                    item_guid,
                    infohash_v1,
                    infohash_v2,
                    magnet_hash,
                    finished_at_input
                FROM eligible
                ON CONFLICT DO NOTHING
                RETURNING 1
            )
            SELECT
                (SELECT count(*) FROM eligible),
                (SELECT count(*) FROM inserted)
            INTO items_eligible, items_inserted;
        END IF;
    END IF;

    IF effective_outcome = 'failure' AND effective_error_class = 'rate_limited' THEN
        items_parsed := 0;
        items_eligible := 0;
        items_inserted := 0;
    END IF;

    IF effective_error_class = 'rate_limited' THEN
        IF rate_limit_denied_scope_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'rate_limit_scope_missing';
        END IF;

        PERFORM outbound_request_log_write_v1(
            instance_public_id_value,
            routing_policy_public_id_value,
            NULL,
            'rss',
            correlation_id_input,
            retry_seq_input,
            now_value,
            now_value,
            'failure',
            via_mitigation_input,
            rate_limit_denied_scope_input,
            'rate_limited',
            http_status_input,
            0,
            FALSE,
            0,
            cf_detected_input,
            NULL,
            NULL
        );
    ELSE
        PERFORM outbound_request_log_write_v1(
            instance_public_id_value,
            routing_policy_public_id_value,
            NULL,
            'rss',
            correlation_id_input,
            retry_seq_input,
            started_at_input,
            finished_at_input,
            effective_outcome,
            via_mitigation_input,
            rate_limit_denied_scope_input,
            CASE WHEN effective_outcome = 'success' THEN NULL ELSE effective_error_class END,
            http_status_input,
            latency_ms_input,
            parse_ok_input,
            result_count_input,
            cf_detected_input,
            NULL,
            NULL
        );
    END IF;

    IF parsed_success THEN
        UPDATE indexer_rss_subscription
        SET last_polled_at = now_value,
            next_poll_at = now_value
                + make_interval(secs => interval_seconds_value + random_jitter_seconds(60)),
            last_error_class = NULL,
            backoff_seconds = NULL
        WHERE indexer_rss_subscription_id = subscription_id_value;

        subscription_succeeded := TRUE;
    ELSE
        subscription_succeeded := FALSE;

        retryable_failure := (
            effective_error_class IN (
                'dns',
                'tls',
                'timeout',
                'connection_refused',
                'http_5xx',
                'http_429',
                'rate_limited'
            )
            OR (effective_error_class = 'cf_challenge' AND cf_retryable_input)
        );

        IF retryable_failure THEN
            IF current_backoff_seconds IS NULL THEN
                new_backoff_seconds := 60;
            ELSE
                new_backoff_seconds := LEAST(current_backoff_seconds * 2, 1800);
            END IF;

            jitter_pct := random_jitter_seconds(25);
            jitter_seconds := (new_backoff_seconds * jitter_pct) / 100;

            UPDATE indexer_rss_subscription
            SET backoff_seconds = new_backoff_seconds,
                last_error_class = effective_error_class,
                next_poll_at = now_value
                    + make_interval(secs => new_backoff_seconds + jitter_seconds)
            WHERE indexer_rss_subscription_id = subscription_id_value;
        ELSE
            UPDATE indexer_rss_subscription
            SET is_enabled = FALSE,
                last_error_class = effective_error_class,
                backoff_seconds = NULL,
                next_poll_at = NULL
            WHERE indexer_rss_subscription_id = subscription_id_value;

            detail_summary := 'RSS subscription auto-disabled: '
                || effective_error_class::TEXT;

            INSERT INTO config_audit_log (
                entity_type,
                entity_pk_bigint,
                entity_public_id,
                action,
                changed_by_user_id,
                change_summary
            )
            VALUES (
                'indexer_instance',
                instance_id_value,
                instance_public_id_value,
                'update',
                0,
                detail_summary
            );
        END IF;
    END IF;

    IF effective_error_class = 'cf_challenge' AND cf_retryable_input = FALSE THEN
        SELECT state, consecutive_failures, backoff_seconds
        INTO cf_state_value, cf_consecutive_failures, cf_backoff_seconds
        FROM indexer_cf_state
        WHERE indexer_instance_id = instance_id_value
        FOR UPDATE OF indexer_cf_state;

        IF cf_state_value IS NULL THEN
            INSERT INTO indexer_cf_state (
                indexer_instance_id,
                state,
                last_changed_at,
                cf_session_id,
                cf_session_expires_at,
                cooldown_until,
                backoff_seconds,
                consecutive_failures,
                last_error_class
            )
            VALUES (
                instance_id_value,
                'challenged',
                now_value,
                NULL,
                NULL,
                NULL,
                NULL,
                1,
                'cf_challenge'
            );
        ELSE
            cf_consecutive_failures := cf_consecutive_failures + 1;

            IF cf_state_value = 'banned' THEN
                UPDATE indexer_cf_state
                SET consecutive_failures = cf_consecutive_failures,
                    last_error_class = 'cf_challenge',
                    last_changed_at = now_value
                WHERE indexer_instance_id = instance_id_value;
            ELSE
                IF cf_consecutive_failures >= 5 THEN
                    IF cf_backoff_seconds IS NULL THEN
                        cf_backoff_seconds := 60;
                    ELSE
                        cf_backoff_seconds := LEAST(cf_backoff_seconds * 2, 21600);
                    END IF;

                    jitter_pct := random_jitter_seconds(25);
                    jitter_seconds := (cf_backoff_seconds * jitter_pct) / 100;

                    UPDATE indexer_cf_state
                    SET state = 'cooldown',
                        last_changed_at = now_value,
                        cooldown_until = now_value
                            + make_interval(secs => cf_backoff_seconds + jitter_seconds),
                        backoff_seconds = cf_backoff_seconds,
                        consecutive_failures = cf_consecutive_failures,
                        last_error_class = 'cf_challenge'
                    WHERE indexer_instance_id = instance_id_value;
                ELSE
                    UPDATE indexer_cf_state
                    SET state = 'challenged',
                        last_changed_at = now_value,
                        consecutive_failures = cf_consecutive_failures,
                        last_error_class = 'cf_challenge'
                    WHERE indexer_instance_id = instance_id_value;
                END IF;
            END IF;
        END IF;
    END IF;

    RETURN NEXT;
    RETURN;
END;
$$;
