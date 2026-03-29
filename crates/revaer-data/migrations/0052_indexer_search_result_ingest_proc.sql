-- Search result ingestion procedure and helpers.

CREATE EXTENSION IF NOT EXISTS unaccent;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'attr_value_type') THEN
        CREATE TYPE attr_value_type AS ENUM (
            'text',
            'int',
            'bigint',
            'numeric',
            'bool',
            'uuid'
        );
    END IF;
END
$$;

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
    value := regexp_replace(value, '\\b(2\\.0|5\\.1|7\\.1)\\b', ' ', 'g');
    value := regexp_replace(value, '[^a-z0-9]+', ' ', 'g');

    FOREACH token IN ARRAY tokens LOOP
        value := regexp_replace(
            value,
            '\\m' || replace(token, ' ', '\\s+') || '\\M',
            ' ',
            'g'
        );
    END LOOP;

    value := regexp_replace(value, '\\s+', ' ', 'g');
    value := btrim(value);

    RETURN value;
END;
$$;

CREATE OR REPLACE FUNCTION normalize_magnet_uri_v1(raw_uri TEXT)
RETURNS TEXT
LANGUAGE plpgsql
AS $$
DECLARE
    trimmed TEXT;
    scheme TEXT;
    query TEXT;
    params TEXT[];
    normalized_parts TEXT[];
BEGIN
    IF raw_uri IS NULL THEN
        RETURN NULL;
    END IF;

    trimmed := btrim(raw_uri);
    IF trimmed = '' THEN
        RETURN NULL;
    END IF;

    scheme := split_part(trimmed, ':', 1);
    IF lower(scheme) <> 'magnet' THEN
        RETURN trimmed;
    END IF;

    IF position('?' IN trimmed) = 0 THEN
        RETURN 'magnet:?';
    END IF;

    query := split_part(trimmed, '?', 2);
    IF query = '' THEN
        RETURN 'magnet:?';
    END IF;

    params := string_to_array(query, '&');

    SELECT array_agg(
        CASE
            WHEN value_part IS NULL THEN key_part
            ELSE key_part || '=' || value_part
        END
        ORDER BY key_part, value_part
    )
    INTO normalized_parts
    FROM (
        SELECT lower(split_part(param, '=', 1)) AS key_part,
               CASE
                   WHEN position('=' IN param) > 0 THEN substring(param FROM position('=' IN param) + 1)
                   ELSE NULL
               END AS value_part
        FROM unnest(params) AS param
        WHERE split_part(param, '=', 1) <> ''
    ) AS parts;

    IF normalized_parts IS NULL THEN
        RETURN 'magnet:?';
    END IF;

    RETURN 'magnet:?' || array_to_string(normalized_parts, '&');
END;
$$;

CREATE OR REPLACE FUNCTION derive_magnet_hash_v1(
    infohash_v2_input TEXT,
    infohash_v1_input TEXT,
    magnet_uri_input TEXT
)
RETURNS TEXT
LANGUAGE plpgsql
AS $$
DECLARE
    normalized_uri TEXT;
    hash_hex TEXT;
BEGIN
    IF infohash_v2_input IS NOT NULL THEN
        hash_hex := encode(digest(decode(infohash_v2_input, 'hex'), 'sha256'), 'hex');
        RETURN lower(hash_hex);
    END IF;

    IF infohash_v1_input IS NOT NULL THEN
        hash_hex := encode(digest(decode(infohash_v1_input, 'hex'), 'sha256'), 'hex');
        RETURN lower(hash_hex);
    END IF;

    IF magnet_uri_input IS NULL THEN
        RETURN NULL;
    END IF;

    normalized_uri := normalize_magnet_uri_v1(magnet_uri_input);
    IF normalized_uri IS NULL THEN
        RETURN NULL;
    END IF;

    hash_hex := encode(digest(normalized_uri, 'sha256'), 'hex');
    RETURN lower(hash_hex);
END;
$$;

CREATE OR REPLACE FUNCTION compute_title_size_hash_v1(
    title_normalized_input TEXT,
    size_bytes_input BIGINT
)
RETURNS TEXT
LANGUAGE plpgsql
AS $$
DECLARE
    hash_hex TEXT;
BEGIN
    IF title_normalized_input IS NULL OR size_bytes_input IS NULL THEN
        RETURN NULL;
    END IF;

    hash_hex := encode(
        digest(title_normalized_input || '|' || size_bytes_input::TEXT, 'sha256'),
        'hex'
    );
    RETURN lower(hash_hex);
END;
$$;

CREATE OR REPLACE FUNCTION policy_text_match_v1(
    candidate_input TEXT,
    match_operator_input policy_match_operator,
    match_value_text_input TEXT,
    value_set_id_input BIGINT,
    is_case_insensitive_input BOOLEAN
)
RETURNS BOOLEAN
LANGUAGE plpgsql
AS $$
DECLARE
    candidate_norm TEXT;
BEGIN
    IF candidate_input IS NULL THEN
        RETURN FALSE;
    END IF;

    IF match_operator_input = 'regex' THEN
        IF match_value_text_input IS NULL THEN
            RETURN FALSE;
        END IF;
        IF is_case_insensitive_input THEN
            RETURN candidate_input ~* match_value_text_input;
        END IF;
        RETURN candidate_input ~ match_value_text_input;
    END IF;

    candidate_norm := lower(candidate_input);

    IF match_operator_input IN ('eq', 'contains', 'starts_with', 'ends_with') THEN
        IF match_value_text_input IS NULL THEN
            RETURN FALSE;
        END IF;
    END IF;

    IF match_operator_input = 'eq' THEN
        RETURN candidate_norm = lower(match_value_text_input);
    ELSIF match_operator_input = 'contains' THEN
        RETURN candidate_norm LIKE '%' || lower(match_value_text_input) || '%';
    ELSIF match_operator_input = 'starts_with' THEN
        RETURN candidate_norm LIKE lower(match_value_text_input) || '%';
    ELSIF match_operator_input = 'ends_with' THEN
        RETURN candidate_norm LIKE '%' || lower(match_value_text_input);
    ELSIF match_operator_input = 'in_set' THEN
        IF value_set_id_input IS NULL THEN
            RETURN FALSE;
        END IF;
        RETURN EXISTS (
            SELECT 1
            FROM policy_rule_value_set_item
            WHERE value_set_id = value_set_id_input
              AND value_text = candidate_norm
        );
    END IF;

    RETURN FALSE;
END;
$$;

CREATE OR REPLACE FUNCTION policy_uuid_match_v1(
    candidate_input UUID,
    match_operator_input policy_match_operator,
    match_value_uuid_input UUID,
    value_set_id_input BIGINT
)
RETURNS BOOLEAN
LANGUAGE plpgsql
AS $$
BEGIN
    IF candidate_input IS NULL THEN
        RETURN FALSE;
    END IF;

    IF match_operator_input = 'eq' THEN
        RETURN candidate_input = match_value_uuid_input;
    ELSIF match_operator_input = 'in_set' THEN
        IF value_set_id_input IS NULL THEN
            RETURN FALSE;
        END IF;
        RETURN EXISTS (
            SELECT 1
            FROM policy_rule_value_set_item
            WHERE value_set_id = value_set_id_input
              AND value_uuid = candidate_input
        );
    END IF;

    RETURN FALSE;
END;
$$;

CREATE OR REPLACE FUNCTION policy_int_match_v1(
    candidate_input INTEGER,
    match_operator_input policy_match_operator,
    match_value_int_input INTEGER,
    value_set_id_input BIGINT
)
RETURNS BOOLEAN
LANGUAGE plpgsql
AS $$
BEGIN
    IF candidate_input IS NULL THEN
        RETURN FALSE;
    END IF;

    IF match_operator_input = 'eq' THEN
        RETURN candidate_input = match_value_int_input;
    ELSIF match_operator_input = 'in_set' THEN
        IF value_set_id_input IS NULL THEN
            RETURN FALSE;
        END IF;
        RETURN EXISTS (
            SELECT 1
            FROM policy_rule_value_set_item
            WHERE value_set_id = value_set_id_input
              AND value_int = candidate_input
        );
    END IF;

    RETURN FALSE;
END;
$$;

CREATE OR REPLACE FUNCTION policy_release_group_match_v1(
    canonical_torrent_id_input BIGINT,
    release_group_token_input TEXT,
    match_operator_input policy_match_operator,
    match_value_text_input TEXT,
    value_set_id_input BIGINT,
    is_case_insensitive_input BOOLEAN
)
RETURNS BOOLEAN
LANGUAGE plpgsql
AS $$
BEGIN
    IF release_group_token_input IS NOT NULL THEN
        IF policy_text_match_v1(
            release_group_token_input,
            match_operator_input,
            match_value_text_input,
            value_set_id_input,
            is_case_insensitive_input
        ) THEN
            RETURN TRUE;
        END IF;
    END IF;

    RETURN EXISTS (
        SELECT 1
        FROM canonical_torrent_signal
        WHERE canonical_torrent_id = canonical_torrent_id_input
          AND signal_key = 'release_group'
          AND policy_text_match_v1(
              value_text,
              match_operator_input,
              match_value_text_input,
              value_set_id_input,
              is_case_insensitive_input
          )
    );
END;
$$;

CREATE OR REPLACE FUNCTION log_source_metadata_conflict_v1(
    canonical_torrent_source_id_input BIGINT,
    indexer_instance_id_input BIGINT,
    conflict_type_input conflict_type,
    existing_value_input TEXT,
    incoming_value_input TEXT,
    observed_at_input TIMESTAMPTZ
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    conflict_id BIGINT;
    existing_value TEXT;
    incoming_value TEXT;
    observed_at_value TIMESTAMPTZ;
BEGIN
    existing_value := COALESCE(existing_value_input, '');
    incoming_value := COALESCE(incoming_value_input, '');

    IF char_length(existing_value) > 256 THEN
        existing_value := substring(existing_value FROM 1 FOR 256);
    END IF;
    IF char_length(incoming_value) > 256 THEN
        incoming_value := substring(incoming_value FROM 1 FOR 256);
    END IF;

    observed_at_value := COALESCE(observed_at_input, now());

    INSERT INTO source_metadata_conflict (
        canonical_torrent_source_id,
        conflict_type,
        existing_value,
        incoming_value,
        observed_at
    )
    VALUES (
        canonical_torrent_source_id_input,
        conflict_type_input,
        existing_value,
        incoming_value,
        observed_at_value
    )
    RETURNING source_metadata_conflict_id INTO conflict_id;

    INSERT INTO source_metadata_conflict_audit_log (
        conflict_id,
        action,
        actor_user_id,
        occurred_at,
        note
    )
    VALUES (
        conflict_id,
        'created',
        0,
        now(),
        NULL
    );

    INSERT INTO indexer_health_event (
        indexer_instance_id,
        occurred_at,
        event_type,
        detail
    )
    VALUES (
        indexer_instance_id_input,
        observed_at_value,
        'identity_conflict',
        conflict_type_input::TEXT
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_result_ingest_v1(
    search_request_public_id_input UUID,
    indexer_instance_public_id_input UUID,
    source_guid_input VARCHAR,
    details_url_input VARCHAR,
    download_url_input VARCHAR,
    magnet_uri_input VARCHAR,
    title_raw_input VARCHAR,
    size_bytes_input BIGINT,
    infohash_v1_input CHAR(40),
    infohash_v2_input CHAR(64),
    magnet_hash_input CHAR(64),
    seeders_input INTEGER,
    leechers_input INTEGER,
    published_at_input TIMESTAMPTZ,
    uploader_input VARCHAR,
    observed_at_input TIMESTAMPTZ,
    attr_keys_input observation_attr_key[],
    attr_types_input attr_value_type[],
    attr_value_text_input VARCHAR[],
    attr_value_int_input INTEGER[],
    attr_value_bigint_input BIGINT[],
    attr_value_numeric_input NUMERIC(12, 4)[],
    attr_value_bool_input BOOLEAN[],
    attr_value_uuid_input UUID[]
)
RETURNS TABLE(
    canonical_torrent_public_id UUID,
    canonical_torrent_source_public_id UUID,
    observation_created BOOLEAN,
    durable_source_created BOOLEAN,
    canonical_changed BOOLEAN
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to ingest search result';
    errcode CONSTANT text := 'P0001';
    request_id BIGINT;
    request_status search_status;
    request_snapshot_id BIGINT;
    request_page_size INTEGER;
    request_effective_domain_id BIGINT;
    request_profile_id BIGINT;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    instance_enabled BOOLEAN;
    instance_migration_state indexer_instance_migration_state;
    instance_trust_tier_key trust_tier_key;
    instance_trust_rank SMALLINT := 0;
    trust_bucket INTEGER := 0;
    signal_confidence_base NUMERIC(4,3) := 0.5;
    observed_at_value TIMESTAMPTZ;
    trimmed_title TEXT;
    title_for_norm TEXT;
    title_normalized_value TEXT;
    source_guid_value TEXT;
    details_url_value TEXT;
    download_url_value TEXT;
    magnet_uri_value TEXT;
    infohash_v1_value TEXT;
    infohash_v2_value TEXT;
    magnet_hash_value TEXT;
    parsed_infohash_v1 TEXT;
    parsed_infohash_v2 TEXT;
    identity_strategy_value identity_strategy;
    identity_confidence_value NUMERIC(4,3);
    title_size_hash_value TEXT;
    disambiguation_type disambiguation_identity_type;
    disambiguation_value TEXT;
    canonical_id BIGINT;
    canonical_public_id UUID;
    canonical_infohash_v1 TEXT;
    canonical_infohash_v2 TEXT;
    canonical_magnet_hash TEXT;
    canonical_inserted BOOLEAN := FALSE;
    source_id BIGINT;
    source_public_id UUID;
    source_inserted BOOLEAN := FALSE;
    observation_id_value BIGINT;
    observation_inserted BOOLEAN := FALSE;
    guid_conflict_value BOOLEAN := FALSE;
    downranked BOOLEAN := FALSE;
    flagged BOOLEAN := FALSE;
    dropped_canonical BOOLEAN := FALSE;
    dropped_source BOOLEAN := FALSE;
    allowlist_scope_indexer INTEGER;
    allowlist_scope_title INTEGER;
    allowlist_scope_release_group INTEGER;
    allowlist_scope_domain INTEGER;
    allowlist_scope_trust INTEGER;
    require_indexer_rule UUID;
    require_title_rule UUID;
    require_release_group_rule UUID;
    require_domain_rule UUID;
    require_trust_rule UUID;
    require_indexer_matched BOOLEAN := FALSE;
    require_title_matched BOOLEAN := FALSE;
    require_release_group_matched BOOLEAN := FALSE;
    require_domain_matched BOOLEAN := FALSE;
    require_trust_matched BOOLEAN := FALSE;
    release_group_token TEXT;
    release_group_confidence NUMERIC(4,3);
    release_group_suffix_present BOOLEAN := FALSE;
    tracker_name_value TEXT;
    language_primary_value TEXT;
    subtitles_primary_value TEXT;
    tracker_category_value INTEGER;
    tracker_subcategory_value INTEGER;
    files_count_value INTEGER;
    size_bytes_reported_value BIGINT;
    imdb_id_value TEXT;
    tmdb_id_value INTEGER;
    tvdb_id_value INTEGER;
    season_value INTEGER;
    episode_value INTEGER;
    year_value INTEGER;
    attr_count INTEGER;
    existing_infohash_v1 TEXT;
    existing_infohash_v2 TEXT;
    existing_magnet_hash TEXT;
    existing_source_guid TEXT;
    existing_tracker_name TEXT;
    existing_tracker_category INTEGER;
    existing_tracker_subcategory INTEGER;
    existing_files_count INTEGER;
    existing_size_bytes_reported BIGINT;
    existing_imdb_id TEXT;
    existing_tmdb_id INTEGER;
    existing_tvdb_id INTEGER;
    existing_season INTEGER;
    existing_episode INTEGER;
    existing_year INTEGER;
    best_imdb_id TEXT;
    best_tmdb_id INTEGER;
    best_tvdb_id INTEGER;
    best_title_display TEXT;
    media_domain_key_value TEXT;
    instance_domain_ids BIGINT[];
    base_score NUMERIC(12,4) := 0;
    score_policy_adjust NUMERIC(12,4) := 0;
    score_tag_adjust NUMERIC(12,4) := 0;
    score_total_context NUMERIC(12,4) := 0;
    best_context_score NUMERIC(12,4);
    best_context_source BIGINT;
    best_context_seeders INTEGER;
    should_update_best BOOLEAN := FALSE;
    page_id BIGINT;
    page_number_value INTEGER;
    page_item_count INTEGER := 0;
    canonical_link_id BIGINT;
    size_sample_allowed BOOLEAN := FALSE;
    size_cutoff BIGINT := 10995116277760; -- 10 TiB
    sample_count INTEGER;
    sample_median NUMERIC(20,4);
    sample_min BIGINT;
    sample_max BIGINT;
    sample_first BIGINT;
    policy_min_rank INTEGER;
    policy_rule_record RECORD;
    rule_matched BOOLEAN;
BEGIN
    IF search_request_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_missing';
    END IF;

    IF indexer_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_missing';
    END IF;

    SELECT search_request_id, status, policy_snapshot_id, page_size, effective_media_domain_id, search_profile_id
    INTO request_id, request_status, request_snapshot_id, request_page_size, request_effective_domain_id, request_profile_id
    FROM search_request
    WHERE search_request_public_id = search_request_public_id_input;

    IF request_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_not_found';
    END IF;

    IF request_status <> 'running' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_not_running';
    END IF;

    SELECT indexer_instance_id, deleted_at, is_enabled, migration_state, trust_tier_key
    INTO instance_id, instance_deleted_at, instance_enabled, instance_migration_state, instance_trust_tier_key
    FROM indexer_instance
    WHERE indexer_instance_public_id = indexer_instance_public_id_input;

    IF instance_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_not_found';
    END IF;

    IF instance_deleted_at IS NOT NULL OR instance_enabled IS NOT TRUE THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_disabled';
    END IF;

    IF instance_migration_state IS NOT NULL AND instance_migration_state <> 'ready' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_not_ready';
    END IF;

    IF NOT EXISTS (
        SELECT 1
        FROM search_request_indexer_run
        WHERE search_request_id = request_id
          AND indexer_instance_id = instance_id
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_instance_not_in_search';
    END IF;

    IF instance_trust_tier_key IS NOT NULL THEN
        SELECT rank
        INTO instance_trust_rank
        FROM trust_tier
        WHERE trust_tier_key = instance_trust_tier_key;
        IF instance_trust_rank IS NULL THEN
            instance_trust_rank := 0;
        END IF;
    END IF;

    IF instance_trust_rank >= 40 THEN
        trust_bucket := 3;
    ELSIF instance_trust_rank >= 30 THEN
        trust_bucket := 2;
    ELSIF instance_trust_rank >= 20 THEN
        trust_bucket := 1;
    ELSE
        trust_bucket := 0;
    END IF;

    signal_confidence_base := 0.5 + (trust_bucket * 0.1);

    trimmed_title := COALESCE(title_raw_input, '');
    trimmed_title := btrim(trimmed_title);
    IF trimmed_title = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'missing_title';
    END IF;

    source_guid_value := NULLIF(btrim(source_guid_input), '');
    details_url_value := NULLIF(btrim(details_url_input), '');
    download_url_value := NULLIF(btrim(download_url_input), '');
    magnet_uri_value := NULLIF(btrim(magnet_uri_input), '');

    infohash_v1_value := NULLIF(lower(infohash_v1_input), '');
    infohash_v2_value := NULLIF(lower(infohash_v2_input), '');
    magnet_hash_value := NULLIF(lower(magnet_hash_input), '');

    IF infohash_v1_value IS NOT NULL AND infohash_v1_value !~ '^[0-9a-f]{40}$' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'invalid_hash';
    END IF;

    IF infohash_v2_value IS NOT NULL AND infohash_v2_value !~ '^[0-9a-f]{64}$' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'invalid_hash';
    END IF;

    IF magnet_hash_value IS NOT NULL AND magnet_hash_value !~ '^[0-9a-f]{64}$' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'invalid_hash';
    END IF;

    IF magnet_uri_value IS NOT NULL THEN
        SELECT lower((regexp_matches(magnet_uri_value, '(?i)xt=urn:btih:([0-9a-f]{40})'))[1])
        INTO parsed_infohash_v1;
        SELECT lower((regexp_matches(magnet_uri_value, '(?i)xt=urn:btmh:(?:1220)?([0-9a-f]{64})'))[1])
        INTO parsed_infohash_v2;

        IF infohash_v1_value IS NULL AND parsed_infohash_v1 IS NOT NULL THEN
            infohash_v1_value := parsed_infohash_v1;
        END IF;

        IF infohash_v2_value IS NULL AND parsed_infohash_v2 IS NOT NULL THEN
            infohash_v2_value := parsed_infohash_v2;
        END IF;
    END IF;

    magnet_hash_value := COALESCE(
        magnet_hash_value,
        derive_magnet_hash_v1(infohash_v2_value, infohash_v1_value, magnet_uri_value)
    );

    IF source_guid_value IS NULL
        AND infohash_v1_value IS NULL
        AND infohash_v2_value IS NULL
        AND magnet_hash_value IS NULL
        AND size_bytes_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'insufficient_identity';
    END IF;

    observed_at_value := COALESCE(observed_at_input, now());

    release_group_suffix_present := FALSE;
    release_group_token := NULL;
    release_group_confidence := 0;
    IF trimmed_title ~* '(\\s-\\s|-)[A-Za-z0-9]{2,20}\\s*$' THEN
        release_group_suffix_present := TRUE;
        SELECT (regexp_matches(trimmed_title, '(?i)(?:\\s-\\s|-)([A-Za-z0-9]{2,20})\\s*$'))[1]
        INTO release_group_token;
        IF release_group_token IS NOT NULL THEN
            release_group_token := lower(release_group_token);
            release_group_confidence := release_group_confidence + 0.6;
            IF release_group_token !~ '\\s' THEN
                release_group_confidence := release_group_confidence + 0.1;
            END IF;
            IF release_group_token IN ('repack', 'proper', 'web') THEN
                release_group_confidence := release_group_confidence - 0.2;
            END IF;
            IF trimmed_title ~* '(2160p|1080p|720p|480p|4320p|4k|8k|webrip|web[- ]?dl|bluray|blu[- ]?ray|bdrip|hdtv|x264|x265|h264|h265|hevc|avc|xvid|divx|vp9|av1)' THEN
                release_group_confidence := release_group_confidence + 0.2;
            END IF;
        END IF;
    END IF;

    IF release_group_suffix_present THEN
        title_for_norm := regexp_replace(trimmed_title, '(?i)(?:\\s-\\s|-)[A-Za-z0-9]{2,20}\\s*$', '', 'g');
        title_for_norm := btrim(title_for_norm);
    ELSE
        title_for_norm := trimmed_title;
    END IF;

    IF release_group_confidence < 0.8 THEN
        release_group_token := NULL;
        release_group_confidence := 0;
    END IF;

    title_normalized_value := normalize_title_v1(title_for_norm);

    IF title_normalized_value IS NULL OR title_normalized_value = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'missing_title';
    END IF;

    IF infohash_v2_value IS NOT NULL THEN
        identity_strategy_value := 'infohash_v2';
        identity_confidence_value := 1.0;
    ELSIF infohash_v1_value IS NOT NULL THEN
        identity_strategy_value := 'infohash_v1';
        identity_confidence_value := 1.0;
    ELSIF magnet_hash_value IS NOT NULL THEN
        identity_strategy_value := 'magnet_hash';
        identity_confidence_value := 0.85;
    ELSE
        identity_strategy_value := 'title_size_fallback';
        identity_confidence_value := 0.60;
        title_size_hash_value := compute_title_size_hash_v1(title_normalized_value, size_bytes_input);
    END IF;

    IF identity_strategy_value = 'title_size_fallback' AND title_size_hash_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'insufficient_identity';
    END IF;

    IF identity_strategy_value = 'infohash_v2' THEN
        disambiguation_type := 'infohash_v2';
        disambiguation_value := infohash_v2_value;
    ELSIF identity_strategy_value = 'infohash_v1' THEN
        disambiguation_type := 'infohash_v1';
        disambiguation_value := infohash_v1_value;
    ELSIF identity_strategy_value = 'magnet_hash' THEN
        disambiguation_type := 'magnet_hash';
        disambiguation_value := magnet_hash_value;
    ELSE
        disambiguation_type := NULL;
        disambiguation_value := NULL;
    END IF;

    IF identity_strategy_value = 'infohash_v2' THEN
        SELECT canonical_torrent_id, canonical_torrent_public_id
        INTO canonical_id, canonical_public_id
        FROM canonical_torrent
        WHERE infohash_v2 = infohash_v2_value;
    ELSIF identity_strategy_value = 'infohash_v1' THEN
        SELECT canonical_torrent_id, canonical_torrent_public_id
        INTO canonical_id, canonical_public_id
        FROM canonical_torrent
        WHERE infohash_v1 = infohash_v1_value;
    ELSIF identity_strategy_value = 'magnet_hash' THEN
        SELECT canonical_torrent_id, canonical_torrent_public_id
        INTO canonical_id, canonical_public_id
        FROM canonical_torrent
        WHERE magnet_hash = magnet_hash_value;
    ELSE
        SELECT canonical_torrent_id, canonical_torrent_public_id
        INTO canonical_id, canonical_public_id
        FROM canonical_torrent
        WHERE title_size_hash = title_size_hash_value;
    END IF;

    IF canonical_id IS NOT NULL
        AND disambiguation_type IS NOT NULL
        AND disambiguation_value IS NOT NULL
        AND EXISTS (
            SELECT 1
            FROM canonical_disambiguation_rule
            WHERE rule_type = 'prevent_merge'
              AND (
                  (
                      identity_left_type = 'canonical_public_id'
                      AND identity_left_value_uuid = canonical_public_id
                      AND identity_right_type = disambiguation_type
                      AND identity_right_value_text = disambiguation_value
                  )
                  OR (
                      identity_right_type = 'canonical_public_id'
                      AND identity_right_value_uuid = canonical_public_id
                      AND identity_left_type = disambiguation_type
                      AND identity_left_value_text = disambiguation_value
                  )
              )
        ) THEN
        canonical_id := NULL;
        canonical_public_id := NULL;
    END IF;

    IF canonical_id IS NULL THEN
        canonical_public_id := gen_random_uuid();
        INSERT INTO canonical_torrent (
            canonical_torrent_public_id,
            identity_confidence,
            identity_strategy,
            infohash_v1,
            infohash_v2,
            magnet_hash,
            title_size_hash,
            title_display,
            title_normalized,
            size_bytes
        )
        VALUES (
            canonical_public_id,
            identity_confidence_value,
            identity_strategy_value,
            infohash_v1_value,
            infohash_v2_value,
            magnet_hash_value,
            title_size_hash_value,
            trimmed_title,
            title_normalized_value,
            CASE
                WHEN identity_strategy_value = 'title_size_fallback' THEN size_bytes_input
                ELSE NULL
            END
        )
        RETURNING canonical_torrent_id INTO canonical_id;
        canonical_inserted := TRUE;
    END IF;

    SELECT infohash_v1, infohash_v2, magnet_hash
    INTO canonical_infohash_v1, canonical_infohash_v2, canonical_magnet_hash
    FROM canonical_torrent
    WHERE canonical_torrent_id = canonical_id;

    IF source_guid_value IS NOT NULL THEN
        SELECT canonical_torrent_source_id, canonical_torrent_source_public_id
        INTO source_id, source_public_id
        FROM canonical_torrent_source
        WHERE indexer_instance_id = instance_id
          AND source_guid = source_guid_value;
    END IF;

    IF source_id IS NULL AND infohash_v2_value IS NOT NULL THEN
        SELECT canonical_torrent_source_id, canonical_torrent_source_public_id
        INTO source_id, source_public_id
        FROM canonical_torrent_source
        WHERE indexer_instance_id = instance_id
          AND source_guid IS NULL
          AND infohash_v2 = infohash_v2_value;
    END IF;

    IF source_id IS NULL AND infohash_v1_value IS NOT NULL THEN
        SELECT canonical_torrent_source_id, canonical_torrent_source_public_id
        INTO source_id, source_public_id
        FROM canonical_torrent_source
        WHERE indexer_instance_id = instance_id
          AND source_guid IS NULL
          AND infohash_v2 IS NULL
          AND infohash_v1 = infohash_v1_value;
    END IF;

    IF source_id IS NULL AND magnet_hash_value IS NOT NULL THEN
        SELECT canonical_torrent_source_id, canonical_torrent_source_public_id
        INTO source_id, source_public_id
        FROM canonical_torrent_source
        WHERE indexer_instance_id = instance_id
          AND source_guid IS NULL
          AND infohash_v2 IS NULL
          AND infohash_v1 IS NULL
          AND magnet_hash = magnet_hash_value;
    END IF;

    IF source_id IS NULL AND size_bytes_input IS NOT NULL THEN
        SELECT canonical_torrent_source_id, canonical_torrent_source_public_id
        INTO source_id, source_public_id
        FROM canonical_torrent_source
        WHERE indexer_instance_id = instance_id
          AND source_guid IS NULL
          AND infohash_v2 IS NULL
          AND infohash_v1 IS NULL
          AND magnet_hash IS NULL
          AND size_bytes = size_bytes_input
          AND title_normalized = title_normalized_value;
    END IF;

    IF source_id IS NULL THEN
        source_public_id := gen_random_uuid();
        INSERT INTO canonical_torrent_source (
            indexer_instance_id,
            canonical_torrent_source_public_id,
            source_guid,
            infohash_v1,
            infohash_v2,
            magnet_hash,
            title_normalized,
            size_bytes,
            last_seen_at,
            last_seen_seeders,
            last_seen_leechers,
            last_seen_published_at,
            last_seen_download_url,
            last_seen_magnet_uri,
            last_seen_details_url,
            last_seen_uploader
        )
        VALUES (
            instance_id,
            source_public_id,
            source_guid_value,
            infohash_v1_value,
            infohash_v2_value,
            magnet_hash_value,
            title_normalized_value,
            size_bytes_input,
            observed_at_value,
            seeders_input,
            leechers_input,
            published_at_input,
            download_url_value,
            magnet_uri_value,
            details_url_value,
            uploader_input
        )
        RETURNING canonical_torrent_source_id INTO source_id;
        source_inserted := TRUE;
    ELSE
        SELECT source_guid, infohash_v1, infohash_v2, magnet_hash
        INTO existing_source_guid, existing_infohash_v1, existing_infohash_v2, existing_magnet_hash
        FROM canonical_torrent_source
        WHERE canonical_torrent_source_id = source_id;

        IF source_guid_value IS NOT NULL THEN
            IF existing_source_guid IS NULL THEN
                IF EXISTS (
                    SELECT 1
                    FROM canonical_torrent_source
                    WHERE indexer_instance_id = instance_id
                      AND source_guid = source_guid_value
                      AND canonical_torrent_source_id <> source_id
                ) THEN
                    SELECT canonical_torrent_source_public_id
                    INTO existing_source_guid
                    FROM canonical_torrent_source
                    WHERE indexer_instance_id = instance_id
                      AND source_guid = source_guid_value
                      AND canonical_torrent_source_id <> source_id
                    LIMIT 1;

                    guid_conflict_value := TRUE;
                    PERFORM log_source_metadata_conflict_v1(
                        source_id,
                        instance_id,
                        'source_guid',
                        existing_source_guid,
                        source_guid_value,
                        observed_at_value
                    );
                ELSE
                    UPDATE canonical_torrent_source
                    SET source_guid = source_guid_value,
                        updated_at = now()
                    WHERE canonical_torrent_source_id = source_id;
                    existing_source_guid := source_guid_value;
                END IF;
            ELSIF existing_source_guid <> source_guid_value THEN
                guid_conflict_value := TRUE;
                PERFORM log_source_metadata_conflict_v1(
                    source_id,
                    instance_id,
                    'source_guid',
                    source_public_id::TEXT,
                    source_guid_value,
                    observed_at_value
                );
            END IF;
        END IF;

        IF infohash_v2_value IS NOT NULL THEN
            IF existing_infohash_v2 IS NULL THEN
                IF EXISTS (
                    SELECT 1
                    FROM canonical_torrent_source
                    WHERE indexer_instance_id = instance_id
                      AND source_guid IS NULL
                      AND infohash_v2 = infohash_v2_value
                      AND canonical_torrent_source_id <> source_id
                ) THEN
                    PERFORM log_source_metadata_conflict_v1(
                        source_id,
                        instance_id,
                        'hash',
                        infohash_v2_value,
                        infohash_v2_value,
                        observed_at_value
                    );
                ELSE
                    UPDATE canonical_torrent_source
                    SET infohash_v2 = infohash_v2_value,
                        updated_at = now()
                    WHERE canonical_torrent_source_id = source_id;
                    existing_infohash_v2 := infohash_v2_value;
                END IF;
            ELSIF existing_infohash_v2 <> infohash_v2_value THEN
                PERFORM log_source_metadata_conflict_v1(
                    source_id,
                    instance_id,
                    'hash',
                    existing_infohash_v2,
                    infohash_v2_value,
                    observed_at_value
                );
            END IF;
        END IF;

        IF infohash_v1_value IS NOT NULL THEN
            IF existing_infohash_v1 IS NULL THEN
                IF existing_infohash_v2 IS NULL AND EXISTS (
                    SELECT 1
                    FROM canonical_torrent_source
                    WHERE indexer_instance_id = instance_id
                      AND source_guid IS NULL
                      AND infohash_v2 IS NULL
                      AND infohash_v1 = infohash_v1_value
                      AND canonical_torrent_source_id <> source_id
                ) THEN
                    PERFORM log_source_metadata_conflict_v1(
                        source_id,
                        instance_id,
                        'hash',
                        infohash_v1_value,
                        infohash_v1_value,
                        observed_at_value
                    );
                ELSE
                    UPDATE canonical_torrent_source
                    SET infohash_v1 = infohash_v1_value,
                        updated_at = now()
                    WHERE canonical_torrent_source_id = source_id;
                    existing_infohash_v1 := infohash_v1_value;
                END IF;
            ELSIF existing_infohash_v1 <> infohash_v1_value THEN
                PERFORM log_source_metadata_conflict_v1(
                    source_id,
                    instance_id,
                    'hash',
                    existing_infohash_v1,
                    infohash_v1_value,
                    observed_at_value
                );
            END IF;
        END IF;

        IF magnet_hash_value IS NOT NULL THEN
            IF existing_magnet_hash IS NULL THEN
                IF existing_infohash_v2 IS NULL AND existing_infohash_v1 IS NULL AND EXISTS (
                    SELECT 1
                    FROM canonical_torrent_source
                    WHERE indexer_instance_id = instance_id
                      AND source_guid IS NULL
                      AND infohash_v2 IS NULL
                      AND infohash_v1 IS NULL
                      AND magnet_hash = magnet_hash_value
                      AND canonical_torrent_source_id <> source_id
                ) THEN
                    PERFORM log_source_metadata_conflict_v1(
                        source_id,
                        instance_id,
                        'hash',
                        magnet_hash_value,
                        magnet_hash_value,
                        observed_at_value
                    );
                ELSE
                    UPDATE canonical_torrent_source
                    SET magnet_hash = magnet_hash_value,
                        updated_at = now()
                    WHERE canonical_torrent_source_id = source_id;
                    existing_magnet_hash := magnet_hash_value;
                END IF;
            ELSIF existing_magnet_hash <> magnet_hash_value THEN
                PERFORM log_source_metadata_conflict_v1(
                    source_id,
                    instance_id,
                    'hash',
                    existing_magnet_hash,
                    magnet_hash_value,
                    observed_at_value
                );
            END IF;
        END IF;

        UPDATE canonical_torrent_source
        SET last_seen_at = CASE
                WHEN observed_at_value > last_seen_at THEN observed_at_value
                ELSE last_seen_at
            END,
            last_seen_seeders = CASE
                WHEN observed_at_value > last_seen_at THEN seeders_input
                ELSE last_seen_seeders
            END,
            last_seen_leechers = CASE
                WHEN observed_at_value > last_seen_at THEN leechers_input
                ELSE last_seen_leechers
            END,
            last_seen_published_at = CASE
                WHEN observed_at_value > last_seen_at THEN published_at_input
                ELSE last_seen_published_at
            END,
            last_seen_download_url = CASE
                WHEN observed_at_value > last_seen_at THEN download_url_value
                ELSE last_seen_download_url
            END,
            last_seen_magnet_uri = CASE
                WHEN observed_at_value > last_seen_at THEN magnet_uri_value
                ELSE last_seen_magnet_uri
            END,
            last_seen_details_url = CASE
                WHEN observed_at_value > last_seen_at THEN details_url_value
                ELSE last_seen_details_url
            END,
            last_seen_uploader = CASE
                WHEN observed_at_value > last_seen_at THEN uploader_input
                ELSE last_seen_uploader
            END,
            updated_at = now()
        WHERE canonical_torrent_source_id = source_id;
    END IF;

    IF attr_keys_input IS NOT NULL THEN
        IF attr_types_input IS NULL
            OR attr_value_text_input IS NULL
            OR attr_value_int_input IS NULL
            OR attr_value_bigint_input IS NULL
            OR attr_value_numeric_input IS NULL
            OR attr_value_bool_input IS NULL
            OR attr_value_uuid_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'attr_length_mismatch';
        END IF;

        attr_count := COALESCE(array_length(attr_keys_input, 1), 0);
        IF attr_count <> COALESCE(array_length(attr_types_input, 1), 0)
            OR attr_count <> COALESCE(array_length(attr_value_text_input, 1), 0)
            OR attr_count <> COALESCE(array_length(attr_value_int_input, 1), 0)
            OR attr_count <> COALESCE(array_length(attr_value_bigint_input, 1), 0)
            OR attr_count <> COALESCE(array_length(attr_value_numeric_input, 1), 0)
            OR attr_count <> COALESCE(array_length(attr_value_bool_input, 1), 0)
            OR attr_count <> COALESCE(array_length(attr_value_uuid_input, 1), 0) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'attr_length_mismatch';
        END IF;

        CREATE TEMP TABLE tmp_attrs (
            attr_key observation_attr_key,
            attr_type attr_value_type,
            value_text VARCHAR,
            value_int INTEGER,
            value_bigint BIGINT,
            value_numeric NUMERIC(12,4),
            value_bool BOOLEAN,
            value_uuid UUID
        ) ON COMMIT DROP;

        INSERT INTO tmp_attrs (attr_key, attr_type, value_text, value_int, value_bigint, value_numeric, value_bool, value_uuid)
        SELECT *
        FROM unnest(
            attr_keys_input,
            attr_types_input,
            attr_value_text_input,
            attr_value_int_input,
            attr_value_bigint_input,
            attr_value_numeric_input,
            attr_value_bool_input,
            attr_value_uuid_input
        );

        IF EXISTS (
            SELECT 1
            FROM tmp_attrs
            GROUP BY attr_key
            HAVING COUNT(*) > 1
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'duplicate_attr_key';
        END IF;

        IF EXISTS (
            SELECT 1
            FROM tmp_attrs
            WHERE (
                (attr_type = 'text' AND value_text IS NULL)
                OR (attr_type = 'int' AND value_int IS NULL)
                OR (attr_type = 'bigint' AND value_bigint IS NULL)
                OR (attr_type = 'numeric' AND value_numeric IS NULL)
                OR (attr_type = 'bool' AND value_bool IS NULL)
                OR (attr_type = 'uuid' AND value_uuid IS NULL)
            )
            OR (
                (value_text IS NOT NULL)::INT
                + (value_int IS NOT NULL)::INT
                + (value_bigint IS NOT NULL)::INT
                + (value_numeric IS NOT NULL)::INT
                + (value_bool IS NOT NULL)::INT
                + (value_uuid IS NOT NULL)::INT
            ) <> 1
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'attr_value_invalid';
        END IF;

        IF EXISTS (
            SELECT 1
            FROM tmp_attrs
            WHERE (
                attr_key IN ('tracker_name', 'release_group', 'language_primary', 'subtitles_primary', 'imdb_id')
                AND attr_type <> 'text'
            )
            OR (
                attr_key = 'size_bytes_reported'
                AND attr_type <> 'bigint'
            )
            OR (
                attr_key IN (
                    'tracker_category',
                    'tracker_subcategory',
                    'files_count',
                    'season',
                    'episode',
                    'year',
                    'tmdb_id',
                    'tvdb_id',
                    'minimum_seed_time_hours'
                ) AND attr_type <> 'int'
            )
            OR (attr_key = 'minimum_ratio' AND attr_type <> 'numeric')
            OR (attr_key IN ('freeleech', 'internal_flag', 'scene_flag') AND attr_type <> 'bool')
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'attr_type_mismatch';
        END IF;

        SELECT value_text INTO tracker_name_value
        FROM tmp_attrs WHERE attr_key = 'tracker_name';
        SELECT value_text INTO language_primary_value
        FROM tmp_attrs WHERE attr_key = 'language_primary';
        SELECT value_text INTO subtitles_primary_value
        FROM tmp_attrs WHERE attr_key = 'subtitles_primary';
        SELECT value_int INTO tracker_category_value
        FROM tmp_attrs WHERE attr_key = 'tracker_category';
        SELECT value_int INTO tracker_subcategory_value
        FROM tmp_attrs WHERE attr_key = 'tracker_subcategory';
        SELECT value_int INTO files_count_value
        FROM tmp_attrs WHERE attr_key = 'files_count';
        SELECT value_bigint INTO size_bytes_reported_value
        FROM tmp_attrs WHERE attr_key = 'size_bytes_reported';
        SELECT value_text INTO imdb_id_value
        FROM tmp_attrs WHERE attr_key = 'imdb_id';
        SELECT value_int INTO tmdb_id_value
        FROM tmp_attrs WHERE attr_key = 'tmdb_id';
        SELECT value_int INTO tvdb_id_value
        FROM tmp_attrs WHERE attr_key = 'tvdb_id';
        SELECT value_int INTO season_value
        FROM tmp_attrs WHERE attr_key = 'season';
        SELECT value_int INTO episode_value
        FROM tmp_attrs WHERE attr_key = 'episode';
        SELECT value_int INTO year_value
        FROM tmp_attrs WHERE attr_key = 'year';

        IF tracker_category_value IS NOT NULL AND tracker_category_value < 0 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'attr_value_invalid';
        END IF;

        IF tracker_subcategory_value IS NOT NULL AND tracker_subcategory_value < 0 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'attr_value_invalid';
        END IF;

        IF files_count_value IS NOT NULL AND files_count_value < 0 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'attr_value_invalid';
        END IF;

        IF size_bytes_reported_value IS NOT NULL AND size_bytes_reported_value < 0 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'attr_value_invalid';
        END IF;

        IF season_value IS NOT NULL AND season_value < 0 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'attr_value_invalid';
        END IF;

        IF episode_value IS NOT NULL AND episode_value < 0 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'attr_value_invalid';
        END IF;

        IF year_value IS NOT NULL AND year_value < 0 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'attr_value_invalid';
        END IF;

        IF tmdb_id_value IS NOT NULL AND tmdb_id_value <= 0 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'attr_value_invalid';
        END IF;

        IF tvdb_id_value IS NOT NULL AND tvdb_id_value <= 0 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'attr_value_invalid';
        END IF;

        IF imdb_id_value IS NOT NULL THEN
            imdb_id_value := lower(imdb_id_value);
            IF imdb_id_value !~ '^tt[0-9]{7,9}$' THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'attr_value_invalid';
            END IF;
            UPDATE tmp_attrs
            SET value_text = imdb_id_value
            WHERE attr_key = 'imdb_id';
        END IF;

        IF language_primary_value IS NOT NULL THEN
            language_primary_value := lower(btrim(language_primary_value));
            IF language_primary_value = '' THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'attr_value_invalid';
            END IF;
            UPDATE tmp_attrs
            SET value_text = language_primary_value
            WHERE attr_key = 'language_primary';
        END IF;

        IF subtitles_primary_value IS NOT NULL THEN
            subtitles_primary_value := lower(btrim(subtitles_primary_value));
            IF subtitles_primary_value = '' THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'attr_value_invalid';
            END IF;
            UPDATE tmp_attrs
            SET value_text = subtitles_primary_value
            WHERE attr_key = 'subtitles_primary';
        END IF;

        IF release_group_confidence < 0.8 THEN
            DELETE FROM tmp_attrs WHERE attr_key = 'release_group';
        ELSIF release_group_token IS NOT NULL THEN
            DELETE FROM tmp_attrs
            WHERE attr_key = 'release_group'
              AND lower(value_text) IS DISTINCT FROM release_group_token;
            UPDATE tmp_attrs
            SET value_text = release_group_token
            WHERE attr_key = 'release_group';
        END IF;
    END IF;

    IF tracker_name_value IS NULL THEN
        SELECT value_text
        INTO tracker_name_value
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = source_id
          AND attr_key = 'tracker_name';
    END IF;

    IF request_snapshot_id IS NOT NULL THEN
        CREATE TEMP TABLE tmp_policy_rules AS
        SELECT psr.rule_order,
               pr.policy_rule_public_id,
               pr.rule_type,
               pr.action,
               pr.severity,
               pr.match_field,
               pr.match_operator,
               pr.match_value_text,
               pr.match_value_int,
               pr.match_value_uuid,
               pr.value_set_id,
               pr.is_case_insensitive,
               ps.scope,
               CASE ps.scope
                   WHEN 'request' THEN 1
                   WHEN 'profile' THEN 2
                   WHEN 'user' THEN 3
                   ELSE 4
               END AS scope_rank
        FROM policy_snapshot_rule psr
        JOIN policy_rule pr
            ON pr.policy_rule_public_id = psr.policy_rule_public_id
        JOIN policy_set ps
            ON ps.policy_set_id = pr.policy_set_id
        WHERE psr.policy_snapshot_id = request_snapshot_id;

        CREATE TEMP TABLE tmp_policy_matches (
            policy_rule_public_id UUID,
            action policy_action,
            severity policy_severity,
            rule_type policy_rule_type
        ) ON COMMIT DROP;

        SELECT MIN(scope_rank)
        INTO allowlist_scope_indexer
        FROM tmp_policy_rules
        WHERE action = 'require'
          AND rule_type = 'allow_indexer_instance';

        SELECT MIN(scope_rank)
        INTO allowlist_scope_title
        FROM tmp_policy_rules
        WHERE action = 'require'
          AND rule_type = 'allow_title_regex';

        SELECT MIN(scope_rank)
        INTO allowlist_scope_release_group
        FROM tmp_policy_rules
        WHERE action = 'require'
          AND rule_type = 'allow_release_group';

        SELECT MIN(scope_rank)
        INTO allowlist_scope_domain
        FROM tmp_policy_rules
        WHERE action = 'require'
          AND rule_type = 'require_media_domain';

        SELECT MIN(scope_rank)
        INTO allowlist_scope_trust
        FROM tmp_policy_rules
        WHERE action = 'require'
          AND rule_type = 'require_trust_tier_min';

        IF allowlist_scope_indexer IS NOT NULL THEN
            SELECT policy_rule_public_id
            INTO require_indexer_rule
            FROM tmp_policy_rules
            WHERE action = 'require'
              AND rule_type = 'allow_indexer_instance'
              AND scope_rank = allowlist_scope_indexer
            ORDER BY rule_order
            LIMIT 1;

            SELECT EXISTS (
                SELECT 1
                FROM tmp_policy_rules
                WHERE action = 'require'
                  AND rule_type = 'allow_indexer_instance'
                  AND scope_rank = allowlist_scope_indexer
                  AND policy_uuid_match_v1(
                      indexer_instance_public_id_input,
                      match_operator,
                      match_value_uuid,
                      value_set_id
                  )
            ) INTO require_indexer_matched;

            IF require_indexer_matched IS NOT TRUE THEN
                dropped_source := TRUE;
            END IF;
        END IF;

        IF allowlist_scope_title IS NOT NULL THEN
            SELECT policy_rule_public_id
            INTO require_title_rule
            FROM tmp_policy_rules
            WHERE action = 'require'
              AND rule_type = 'allow_title_regex'
              AND scope_rank = allowlist_scope_title
            ORDER BY rule_order
            LIMIT 1;

            SELECT EXISTS (
                SELECT 1
                FROM tmp_policy_rules
                WHERE action = 'require'
                  AND rule_type = 'allow_title_regex'
                  AND scope_rank = allowlist_scope_title
                  AND policy_text_match_v1(
                      title_normalized_value,
                      match_operator,
                      match_value_text,
                      value_set_id,
                      is_case_insensitive
                  )
            ) INTO require_title_matched;

            IF require_title_matched IS NOT TRUE THEN
                dropped_canonical := TRUE;
            END IF;
        END IF;

        IF allowlist_scope_release_group IS NOT NULL THEN
            SELECT policy_rule_public_id
            INTO require_release_group_rule
            FROM tmp_policy_rules
            WHERE action = 'require'
              AND rule_type = 'allow_release_group'
              AND scope_rank = allowlist_scope_release_group
            ORDER BY rule_order
            LIMIT 1;

            SELECT EXISTS (
                SELECT 1
                FROM tmp_policy_rules
                WHERE action = 'require'
                  AND rule_type = 'allow_release_group'
                  AND scope_rank = allowlist_scope_release_group
                  AND policy_release_group_match_v1(
                      canonical_id,
                      release_group_token,
                      match_operator,
                      match_value_text,
                      value_set_id,
                      is_case_insensitive
                  )
            ) INTO require_release_group_matched;

            IF require_release_group_matched IS NOT TRUE THEN
                dropped_canonical := TRUE;
            END IF;
        END IF;

        IF allowlist_scope_domain IS NOT NULL THEN
            SELECT policy_rule_public_id
            INTO require_domain_rule
            FROM tmp_policy_rules
            WHERE action = 'require'
              AND rule_type = 'require_media_domain'
              AND scope_rank = allowlist_scope_domain
            ORDER BY rule_order
            LIMIT 1;

            SELECT EXISTS (
                SELECT 1
                FROM tmp_policy_rules pr
                JOIN indexer_instance_media_domain imd
                    ON imd.indexer_instance_id = instance_id
                JOIN media_domain md
                    ON md.media_domain_id = imd.media_domain_id
                WHERE pr.action = 'require'
                  AND pr.rule_type = 'require_media_domain'
                  AND pr.scope_rank = allowlist_scope_domain
                  AND policy_text_match_v1(
                      md.media_domain_key::TEXT,
                      pr.match_operator,
                      pr.match_value_text,
                      pr.value_set_id,
                      pr.is_case_insensitive
                  )
            ) INTO require_domain_matched;

            IF require_domain_matched IS NOT TRUE THEN
                dropped_source := TRUE;
            END IF;
        END IF;

        IF allowlist_scope_trust IS NOT NULL THEN
            SELECT policy_rule_public_id
            INTO require_trust_rule
            FROM tmp_policy_rules
            WHERE action = 'require'
              AND rule_type = 'require_trust_tier_min'
              AND scope_rank = allowlist_scope_trust
            ORDER BY rule_order
            LIMIT 1;

            SELECT MAX(match_value_int)
            INTO policy_min_rank
            FROM tmp_policy_rules
            WHERE action = 'require'
              AND rule_type = 'require_trust_tier_min'
              AND scope_rank = allowlist_scope_trust
              AND match_value_int IS NOT NULL;

            IF policy_min_rank IS NULL THEN
                require_trust_matched := TRUE;
            ELSE
                require_trust_matched := instance_trust_rank >= policy_min_rank;
                IF require_trust_matched IS NOT TRUE THEN
                    dropped_source := TRUE;
                END IF;
            END IF;
        END IF;

        FOR policy_rule_record IN
            SELECT *
            FROM tmp_policy_rules
            ORDER BY rule_order
        LOOP
            IF policy_rule_record.action = 'require' THEN
                CONTINUE;
            END IF;

            rule_matched := FALSE;

            IF policy_rule_record.match_field = 'infohash_v1' THEN
                rule_matched := policy_text_match_v1(
                    canonical_infohash_v1,
                    policy_rule_record.match_operator,
                    policy_rule_record.match_value_text,
                    policy_rule_record.value_set_id,
                    policy_rule_record.is_case_insensitive
                );
            ELSIF policy_rule_record.match_field = 'infohash_v2' THEN
                rule_matched := policy_text_match_v1(
                    canonical_infohash_v2,
                    policy_rule_record.match_operator,
                    policy_rule_record.match_value_text,
                    policy_rule_record.value_set_id,
                    policy_rule_record.is_case_insensitive
                );
            ELSIF policy_rule_record.match_field = 'magnet_hash' THEN
                rule_matched := policy_text_match_v1(
                    canonical_magnet_hash,
                    policy_rule_record.match_operator,
                    policy_rule_record.match_value_text,
                    policy_rule_record.value_set_id,
                    policy_rule_record.is_case_insensitive
                );
            ELSIF policy_rule_record.match_field = 'title' THEN
                rule_matched := policy_text_match_v1(
                    title_normalized_value,
                    policy_rule_record.match_operator,
                    policy_rule_record.match_value_text,
                    policy_rule_record.value_set_id,
                    policy_rule_record.is_case_insensitive
                );
            ELSIF policy_rule_record.match_field = 'release_group' THEN
                rule_matched := policy_release_group_match_v1(
                    canonical_id,
                    release_group_token,
                    policy_rule_record.match_operator,
                    policy_rule_record.match_value_text,
                    policy_rule_record.value_set_id,
                    policy_rule_record.is_case_insensitive
                );
            ELSIF policy_rule_record.match_field = 'uploader' THEN
                rule_matched := policy_text_match_v1(
                    uploader_input,
                    policy_rule_record.match_operator,
                    policy_rule_record.match_value_text,
                    policy_rule_record.value_set_id,
                    policy_rule_record.is_case_insensitive
                );
            ELSIF policy_rule_record.match_field = 'tracker' THEN
                rule_matched := policy_text_match_v1(
                    tracker_name_value,
                    policy_rule_record.match_operator,
                    policy_rule_record.match_value_text,
                    policy_rule_record.value_set_id,
                    policy_rule_record.is_case_insensitive
                );
            ELSIF policy_rule_record.match_field = 'indexer_instance_public_id' THEN
                rule_matched := policy_uuid_match_v1(
                    indexer_instance_public_id_input,
                    policy_rule_record.match_operator,
                    policy_rule_record.match_value_uuid,
                    policy_rule_record.value_set_id
                );
            ELSIF policy_rule_record.match_field = 'media_domain_key' THEN
                rule_matched := EXISTS (
                    SELECT 1
                    FROM indexer_instance_media_domain imd
                    JOIN media_domain md
                        ON md.media_domain_id = imd.media_domain_id
                    WHERE imd.indexer_instance_id = instance_id
                      AND policy_text_match_v1(
                          md.media_domain_key::TEXT,
                          policy_rule_record.match_operator,
                          policy_rule_record.match_value_text,
                          policy_rule_record.value_set_id,
                          policy_rule_record.is_case_insensitive
                      )
                );
            ELSIF policy_rule_record.match_field = 'trust_tier_key' THEN
                rule_matched := policy_text_match_v1(
                    instance_trust_tier_key::TEXT,
                    policy_rule_record.match_operator,
                    policy_rule_record.match_value_text,
                    policy_rule_record.value_set_id,
                    policy_rule_record.is_case_insensitive
                );
            ELSIF policy_rule_record.match_field = 'trust_tier_rank' THEN
                rule_matched := policy_int_match_v1(
                    instance_trust_rank,
                    policy_rule_record.match_operator,
                    policy_rule_record.match_value_int,
                    policy_rule_record.value_set_id
                );
            END IF;

            IF rule_matched THEN
                IF policy_rule_record.action IN ('drop_canonical', 'drop_source', 'downrank', 'flag') THEN
                    INSERT INTO tmp_policy_matches (
                        policy_rule_public_id,
                        action,
                        severity,
                        rule_type
                    )
                    VALUES (
                        policy_rule_record.policy_rule_public_id,
                        policy_rule_record.action,
                        policy_rule_record.severity,
                        policy_rule_record.rule_type
                    );
                END IF;

                IF policy_rule_record.action = 'drop_canonical' THEN
                    dropped_canonical := TRUE;
                ELSIF policy_rule_record.action = 'drop_source' THEN
                    dropped_source := TRUE;
                ELSIF policy_rule_record.action = 'downrank' THEN
                    downranked := TRUE;
                    score_policy_adjust := score_policy_adjust + CASE policy_rule_record.severity
                        WHEN 'hard' THEN -50
                        WHEN 'soft' THEN -10
                        ELSE -25
                    END;
                ELSIF policy_rule_record.action = 'flag' THEN
                    flagged := TRUE;
                ELSIF policy_rule_record.action = 'prefer' THEN
                    IF policy_rule_record.rule_type = 'prefer_indexer_instance' THEN
                        score_policy_adjust := score_policy_adjust + 15;
                    ELSIF policy_rule_record.rule_type = 'prefer_trust_tier' THEN
                        score_policy_adjust := score_policy_adjust + 10;
                    ELSIF policy_rule_record.rule_type = 'allow_title_regex' THEN
                        score_policy_adjust := score_policy_adjust + 8;
                    ELSIF policy_rule_record.rule_type = 'allow_release_group' THEN
                        score_policy_adjust := score_policy_adjust + 8;
                    END IF;
                END IF;
            END IF;
        END LOOP;
    END IF;

    IF dropped_canonical OR dropped_source THEN
        score_total_context := -10000;
    ELSE
        SELECT score_total_base
        INTO base_score
        FROM canonical_torrent_source_base_score
        WHERE canonical_torrent_id = canonical_id
          AND canonical_torrent_source_id = source_id;

        base_score := COALESCE(base_score, 0);

        IF request_profile_id IS NOT NULL THEN
            SELECT COALESCE(SUM(weight_override), 0)
            INTO score_tag_adjust
            FROM search_profile_tag_prefer stp
            JOIN indexer_instance_tag it
                ON it.tag_id = stp.tag_id
            WHERE stp.search_profile_id = request_profile_id
              AND it.indexer_instance_id = instance_id;
        END IF;

        IF score_tag_adjust < -15 THEN
            score_tag_adjust := -15;
        ELSIF score_tag_adjust > 15 THEN
            score_tag_adjust := 15;
        END IF;

        score_total_context := base_score + score_policy_adjust + score_tag_adjust;
        IF score_total_context < -10000 THEN
            score_total_context := -10000;
        ELSIF score_total_context > 10000 THEN
            score_total_context := 10000;
        END IF;
    END IF;

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
    VALUES (
        'search_request',
        request_id,
        canonical_id,
        source_id,
        score_total_context,
        score_policy_adjust,
        score_tag_adjust,
        (dropped_canonical OR dropped_source),
        now()
    )
    ON CONFLICT (context_key_type, context_key_id, canonical_torrent_id, canonical_torrent_source_id)
    DO UPDATE SET
        score_total_context = EXCLUDED.score_total_context,
        score_policy_adjust = EXCLUDED.score_policy_adjust,
        score_tag_adjust = EXCLUDED.score_tag_adjust,
        is_dropped = EXCLUDED.is_dropped,
        computed_at = EXCLUDED.computed_at;

    IF NOT dropped_canonical AND NOT dropped_source THEN
        SELECT score_total_context, canonical_torrent_source_id
        INTO best_context_score, best_context_source
        FROM canonical_torrent_source_context_score
        WHERE context_key_type = 'search_request'
          AND context_key_id = request_id
          AND canonical_torrent_id = canonical_id
        ORDER BY score_total_context DESC, canonical_torrent_source_id ASC
        LIMIT 1;

        should_update_best := FALSE;
        IF best_context_source IS NULL THEN
            should_update_best := TRUE;
        ELSE
            IF score_total_context >= best_context_score + 2 THEN
                should_update_best := TRUE;
            ELSE
                SELECT last_seen_seeders
                INTO best_context_seeders
                FROM canonical_torrent_source
                WHERE canonical_torrent_source_id = best_context_source;

                IF seeders_input IS NOT NULL
                    AND best_context_seeders IS NOT NULL
                    AND best_context_seeders >= 20
                    AND best_context_seeders < 100
                    AND seeders_input >= 100 THEN
                    should_update_best := TRUE;
                END IF;
            END IF;
        END IF;

        IF should_update_best THEN
            INSERT INTO canonical_torrent_best_source_context (
                context_key_type,
                context_key_id,
                canonical_torrent_id,
                canonical_torrent_source_id,
                computed_at
            )
            VALUES (
                'search_request',
                request_id,
                canonical_id,
                source_id,
                now()
            )
            ON CONFLICT (context_key_type, context_key_id, canonical_torrent_id)
            DO UPDATE SET
                canonical_torrent_source_id = EXCLUDED.canonical_torrent_source_id,
                computed_at = EXCLUDED.computed_at;
        END IF;
    END IF;

    SELECT observation_id
    INTO observation_id_value
    FROM search_request_source_observation
    WHERE search_request_id = request_id
      AND indexer_instance_id = instance_id
      AND (
          (source_guid_value IS NOT NULL AND source_guid = source_guid_value)
          OR (source_guid_value IS NULL AND canonical_torrent_source_id = source_id AND source_guid IS NULL)
      )
    LIMIT 1;

    IF observation_id_value IS NULL AND source_guid_value IS NOT NULL THEN
        SELECT observation_id
        INTO observation_id_value
        FROM search_request_source_observation
        WHERE search_request_id = request_id
          AND indexer_instance_id = instance_id
          AND source_guid IS NULL
          AND canonical_torrent_source_id = source_id
        LIMIT 1;
    END IF;

    IF observation_id_value IS NULL THEN
        INSERT INTO search_request_source_observation (
            search_request_id,
            indexer_instance_id,
            canonical_torrent_id,
            canonical_torrent_source_id,
            observed_at,
            seeders,
            leechers,
            published_at,
            uploader,
            source_guid,
            details_url,
            download_url,
            magnet_uri,
            title_raw,
            size_bytes,
            infohash_v1,
            infohash_v2,
            magnet_hash,
            guid_conflict,
            was_downranked,
            was_flagged
        )
        VALUES (
            request_id,
            instance_id,
            canonical_id,
            source_id,
            observed_at_value,
            seeders_input,
            leechers_input,
            published_at_input,
            uploader_input,
            source_guid_value,
            details_url_value,
            download_url_value,
            magnet_uri_value,
            trimmed_title,
            size_bytes_input,
            infohash_v1_value,
            infohash_v2_value,
            magnet_hash_value,
            guid_conflict_value,
            downranked,
            flagged
        )
        RETURNING observation_id INTO observation_id_value;
        observation_inserted := TRUE;
    ELSE
        UPDATE search_request_source_observation
        SET canonical_torrent_id = canonical_id,
            canonical_torrent_source_id = source_id,
            observed_at = observed_at_value,
            seeders = seeders_input,
            leechers = leechers_input,
            published_at = published_at_input,
            uploader = uploader_input,
            source_guid = COALESCE(source_guid, source_guid_value),
            details_url = details_url_value,
            download_url = download_url_value,
            magnet_uri = magnet_uri_value,
            title_raw = trimmed_title,
            size_bytes = size_bytes_input,
            infohash_v1 = infohash_v1_value,
            infohash_v2 = infohash_v2_value,
            magnet_hash = magnet_hash_value,
            guid_conflict = guid_conflict_value,
            was_downranked = downranked,
            was_flagged = flagged
        WHERE observation_id = observation_id_value;
        observation_inserted := FALSE;
    END IF;

    SELECT obs.title_raw
    INTO best_title_display
    FROM search_request_source_observation obs
    JOIN indexer_instance inst
        ON inst.indexer_instance_id = obs.indexer_instance_id
    LEFT JOIN trust_tier tt
        ON tt.trust_tier_key = inst.trust_tier_key
    LEFT JOIN canonical_torrent_source_base_score bs
        ON bs.canonical_torrent_id = canonical_id
       AND bs.canonical_torrent_source_id = obs.canonical_torrent_source_id
    WHERE obs.canonical_torrent_id = canonical_id
    ORDER BY COALESCE(tt.rank, 0) DESC,
             COALESCE(bs.score_total_base, 0) DESC,
             obs.observed_at DESC,
             obs.observation_id ASC
    LIMIT 1;

    IF best_title_display IS NOT NULL THEN
        UPDATE canonical_torrent
        SET title_display = best_title_display,
            updated_at = now()
        WHERE canonical_torrent_id = canonical_id;
    END IF;

    IF attr_keys_input IS NOT NULL THEN
        INSERT INTO search_request_source_observation_attr (
            observation_id,
            attr_key,
            value_text,
            value_int,
            value_bigint,
            value_numeric,
            value_bool,
            value_uuid
        )
        SELECT observation_id_value,
               attr_key,
               value_text,
               value_int,
               value_bigint,
               value_numeric,
               value_bool,
               value_uuid
        FROM tmp_attrs
        ON CONFLICT (observation_id, attr_key)
        DO UPDATE SET
            value_text = EXCLUDED.value_text,
            value_int = EXCLUDED.value_int,
            value_bigint = EXCLUDED.value_bigint,
            value_numeric = EXCLUDED.value_numeric,
            value_bool = EXCLUDED.value_bool,
            value_uuid = EXCLUDED.value_uuid;

        SELECT value_text
        INTO existing_tracker_name
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = source_id
          AND attr_key = 'tracker_name';

        SELECT value_int
        INTO existing_tracker_category
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = source_id
          AND attr_key = 'tracker_category';

        SELECT value_int
        INTO existing_tracker_subcategory
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = source_id
          AND attr_key = 'tracker_subcategory';

        SELECT value_int
        INTO existing_files_count
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = source_id
          AND attr_key = 'files_count';

        SELECT value_bigint
        INTO existing_size_bytes_reported
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = source_id
          AND attr_key = 'size_bytes_reported';

        SELECT value_text
        INTO existing_imdb_id
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = source_id
          AND attr_key = 'imdb_id';

        SELECT value_int
        INTO existing_tmdb_id
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = source_id
          AND attr_key = 'tmdb_id';

        SELECT value_int
        INTO existing_tvdb_id
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = source_id
          AND attr_key = 'tvdb_id';

        SELECT value_int
        INTO existing_season
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = source_id
          AND attr_key = 'season';

        SELECT value_int
        INTO existing_episode
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = source_id
          AND attr_key = 'episode';

        SELECT value_int
        INTO existing_year
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = source_id
          AND attr_key = 'year';

        IF tracker_name_value IS NOT NULL THEN
            IF existing_tracker_name IS NULL THEN
                INSERT INTO canonical_torrent_source_attr (
                    canonical_torrent_source_id,
                    attr_key,
                    value_text
                )
                VALUES (
                    source_id,
                    'tracker_name',
                    tracker_name_value
                )
                ON CONFLICT (canonical_torrent_source_id, attr_key)
                DO UPDATE SET
                    value_text = COALESCE(canonical_torrent_source_attr.value_text, EXCLUDED.value_text);
            ELSIF existing_tracker_name <> tracker_name_value THEN
                PERFORM log_source_metadata_conflict_v1(
                    source_id,
                    instance_id,
                    'tracker_name',
                    existing_tracker_name,
                    tracker_name_value,
                    observed_at_value
                );
            END IF;
        END IF;

        IF tracker_category_value IS NOT NULL THEN
            IF existing_tracker_category IS NULL THEN
                INSERT INTO canonical_torrent_source_attr (
                    canonical_torrent_source_id,
                    attr_key,
                    value_int
                )
                VALUES (
                    source_id,
                    'tracker_category',
                    tracker_category_value
                )
                ON CONFLICT (canonical_torrent_source_id, attr_key)
                DO UPDATE SET
                    value_int = COALESCE(canonical_torrent_source_attr.value_int, EXCLUDED.value_int);
            ELSIF existing_tracker_category <> tracker_category_value THEN
                PERFORM log_source_metadata_conflict_v1(
                    source_id,
                    instance_id,
                    'tracker_category',
                    existing_tracker_category::TEXT,
                    tracker_category_value::TEXT,
                    observed_at_value
                );
            END IF;
        END IF;

        IF tracker_subcategory_value IS NOT NULL THEN
            IF existing_tracker_subcategory IS NULL THEN
                INSERT INTO canonical_torrent_source_attr (
                    canonical_torrent_source_id,
                    attr_key,
                    value_int
                )
                VALUES (
                    source_id,
                    'tracker_subcategory',
                    tracker_subcategory_value
                )
                ON CONFLICT (canonical_torrent_source_id, attr_key)
                DO UPDATE SET
                    value_int = COALESCE(canonical_torrent_source_attr.value_int, EXCLUDED.value_int);
            ELSIF existing_tracker_subcategory <> tracker_subcategory_value THEN
                PERFORM log_source_metadata_conflict_v1(
                    source_id,
                    instance_id,
                    'tracker_category',
                    existing_tracker_subcategory::TEXT,
                    tracker_subcategory_value::TEXT,
                    observed_at_value
                );
            END IF;
        END IF;

        IF size_bytes_reported_value IS NOT NULL THEN
            IF existing_size_bytes_reported IS NULL THEN
                INSERT INTO canonical_torrent_source_attr (
                    canonical_torrent_source_id,
                    attr_key,
                    value_bigint
                )
                VALUES (
                    source_id,
                    'size_bytes_reported',
                    size_bytes_reported_value
                )
                ON CONFLICT (canonical_torrent_source_id, attr_key)
                DO UPDATE SET
                    value_bigint = COALESCE(canonical_torrent_source_attr.value_bigint, EXCLUDED.value_bigint);
            END IF;
        END IF;

        IF files_count_value IS NOT NULL THEN
            IF existing_files_count IS NULL THEN
                INSERT INTO canonical_torrent_source_attr (
                    canonical_torrent_source_id,
                    attr_key,
                    value_int
                )
                VALUES (
                    source_id,
                    'files_count',
                    files_count_value
                )
                ON CONFLICT (canonical_torrent_source_id, attr_key)
                DO UPDATE SET
                    value_int = COALESCE(canonical_torrent_source_attr.value_int, EXCLUDED.value_int);
            END IF;
        END IF;

        IF imdb_id_value IS NOT NULL THEN
            IF existing_imdb_id IS NULL THEN
                INSERT INTO canonical_torrent_source_attr (
                    canonical_torrent_source_id,
                    attr_key,
                    value_text
                )
                VALUES (
                    source_id,
                    'imdb_id',
                    imdb_id_value
                )
                ON CONFLICT (canonical_torrent_source_id, attr_key)
                DO UPDATE SET
                    value_text = COALESCE(canonical_torrent_source_attr.value_text, EXCLUDED.value_text);
            ELSIF existing_imdb_id <> imdb_id_value THEN
                PERFORM log_source_metadata_conflict_v1(
                    source_id,
                    instance_id,
                    'external_id',
                    existing_imdb_id,
                    imdb_id_value,
                    observed_at_value
                );
            END IF;
        END IF;

        IF tmdb_id_value IS NOT NULL THEN
            IF existing_tmdb_id IS NULL THEN
                INSERT INTO canonical_torrent_source_attr (
                    canonical_torrent_source_id,
                    attr_key,
                    value_int
                )
                VALUES (
                    source_id,
                    'tmdb_id',
                    tmdb_id_value
                )
                ON CONFLICT (canonical_torrent_source_id, attr_key)
                DO UPDATE SET
                    value_int = COALESCE(canonical_torrent_source_attr.value_int, EXCLUDED.value_int);
            ELSIF existing_tmdb_id <> tmdb_id_value THEN
                PERFORM log_source_metadata_conflict_v1(
                    source_id,
                    instance_id,
                    'external_id',
                    existing_tmdb_id::TEXT,
                    tmdb_id_value::TEXT,
                    observed_at_value
                );
            END IF;
        END IF;

        IF tvdb_id_value IS NOT NULL THEN
            IF existing_tvdb_id IS NULL THEN
                INSERT INTO canonical_torrent_source_attr (
                    canonical_torrent_source_id,
                    attr_key,
                    value_int
                )
                VALUES (
                    source_id,
                    'tvdb_id',
                    tvdb_id_value
                )
                ON CONFLICT (canonical_torrent_source_id, attr_key)
                DO UPDATE SET
                    value_int = COALESCE(canonical_torrent_source_attr.value_int, EXCLUDED.value_int);
            ELSIF existing_tvdb_id <> tvdb_id_value THEN
                PERFORM log_source_metadata_conflict_v1(
                    source_id,
                    instance_id,
                    'external_id',
                    existing_tvdb_id::TEXT,
                    tvdb_id_value::TEXT,
                    observed_at_value
                );
            END IF;
        END IF;

        IF season_value IS NOT NULL THEN
            IF existing_season IS NULL THEN
                INSERT INTO canonical_torrent_source_attr (
                    canonical_torrent_source_id,
                    attr_key,
                    value_int
                )
                VALUES (
                    source_id,
                    'season',
                    season_value
                )
                ON CONFLICT (canonical_torrent_source_id, attr_key)
                DO UPDATE SET
                    value_int = COALESCE(canonical_torrent_source_attr.value_int, EXCLUDED.value_int);
            END IF;
        END IF;

        IF episode_value IS NOT NULL THEN
            IF existing_episode IS NULL THEN
                INSERT INTO canonical_torrent_source_attr (
                    canonical_torrent_source_id,
                    attr_key,
                    value_int
                )
                VALUES (
                    source_id,
                    'episode',
                    episode_value
                )
                ON CONFLICT (canonical_torrent_source_id, attr_key)
                DO UPDATE SET
                    value_int = COALESCE(canonical_torrent_source_attr.value_int, EXCLUDED.value_int);
            END IF;
        END IF;

        IF year_value IS NOT NULL THEN
            IF existing_year IS NULL THEN
                INSERT INTO canonical_torrent_source_attr (
                    canonical_torrent_source_id,
                    attr_key,
                    value_int
                )
                VALUES (
                    source_id,
                    'year',
                    year_value
                )
                ON CONFLICT (canonical_torrent_source_id, attr_key)
                DO UPDATE SET
                    value_int = COALESCE(canonical_torrent_source_attr.value_int, EXCLUDED.value_int);
            END IF;
        END IF;
    END IF;

    IF release_group_token IS NOT NULL AND release_group_confidence >= 0.8 THEN
        INSERT INTO canonical_torrent_signal (
            canonical_torrent_id,
            signal_key,
            value_text,
            confidence
        )
        VALUES (
            canonical_id,
            'release_group',
            release_group_token,
            release_group_confidence
        )
        ON CONFLICT (canonical_torrent_id, signal_key, value_text, value_int)
        DO UPDATE SET
            confidence = LEAST(
                1.0,
                GREATEST(canonical_torrent_signal.confidence, EXCLUDED.confidence) + 0.05
            );
    END IF;

    IF language_primary_value IS NOT NULL THEN
        INSERT INTO canonical_torrent_signal (
            canonical_torrent_id,
            signal_key,
            value_text,
            confidence
        )
        VALUES (
            canonical_id,
            'language',
            language_primary_value,
            signal_confidence_base
        )
        ON CONFLICT (canonical_torrent_id, signal_key, value_text, value_int)
        DO UPDATE SET
            confidence = LEAST(
                1.0,
                GREATEST(canonical_torrent_signal.confidence, EXCLUDED.confidence) + 0.05
            );
    END IF;

    IF subtitles_primary_value IS NOT NULL THEN
        INSERT INTO canonical_torrent_signal (
            canonical_torrent_id,
            signal_key,
            value_text,
            confidence
        )
        VALUES (
            canonical_id,
            'subtitles',
            subtitles_primary_value,
            signal_confidence_base
        )
        ON CONFLICT (canonical_torrent_id, signal_key, value_text, value_int)
        DO UPDATE SET
            confidence = LEAST(
                1.0,
                GREATEST(canonical_torrent_signal.confidence, EXCLUDED.confidence) + 0.05
            );
    END IF;

    IF year_value IS NOT NULL THEN
        INSERT INTO canonical_torrent_signal (
            canonical_torrent_id,
            signal_key,
            value_int,
            confidence
        )
        VALUES (
            canonical_id,
            'year',
            year_value,
            signal_confidence_base
        )
        ON CONFLICT (canonical_torrent_id, signal_key, value_text, value_int)
        DO UPDATE SET
            confidence = LEAST(
                1.0,
                GREATEST(canonical_torrent_signal.confidence, EXCLUDED.confidence) + 0.05
            );
    END IF;

    IF season_value IS NOT NULL THEN
        INSERT INTO canonical_torrent_signal (
            canonical_torrent_id,
            signal_key,
            value_int,
            confidence
        )
        VALUES (
            canonical_id,
            'season',
            season_value,
            signal_confidence_base
        )
        ON CONFLICT (canonical_torrent_id, signal_key, value_text, value_int)
        DO UPDATE SET
            confidence = LEAST(
                1.0,
                GREATEST(canonical_torrent_signal.confidence, EXCLUDED.confidence) + 0.05
            );
    END IF;

    IF episode_value IS NOT NULL THEN
        INSERT INTO canonical_torrent_signal (
            canonical_torrent_id,
            signal_key,
            value_int,
            confidence
        )
        VALUES (
            canonical_id,
            'episode',
            episode_value,
            signal_confidence_base
        )
        ON CONFLICT (canonical_torrent_id, signal_key, value_text, value_int)
        DO UPDATE SET
            confidence = LEAST(
                1.0,
                GREATEST(canonical_torrent_signal.confidence, EXCLUDED.confidence) + 0.05
            );
    END IF;

    IF imdb_id_value IS NOT NULL THEN
        INSERT INTO canonical_external_id (
            canonical_torrent_id,
            id_type,
            id_value_text,
            trust_tier_rank,
            first_seen_at,
            last_seen_at,
            source_canonical_torrent_source_id
        )
        VALUES (
            canonical_id,
            'imdb',
            lower(imdb_id_value),
            instance_trust_rank,
            observed_at_value,
            observed_at_value,
            source_id
        )
        ON CONFLICT (canonical_torrent_id, id_type, id_value_text)
        DO UPDATE SET
            last_seen_at = EXCLUDED.last_seen_at,
            trust_tier_rank = GREATEST(canonical_external_id.trust_tier_rank, EXCLUDED.trust_tier_rank);
    END IF;

    IF tmdb_id_value IS NOT NULL THEN
        INSERT INTO canonical_external_id (
            canonical_torrent_id,
            id_type,
            id_value_int,
            trust_tier_rank,
            first_seen_at,
            last_seen_at,
            source_canonical_torrent_source_id
        )
        VALUES (
            canonical_id,
            'tmdb',
            tmdb_id_value,
            instance_trust_rank,
            observed_at_value,
            observed_at_value,
            source_id
        )
        ON CONFLICT (canonical_torrent_id, id_type, id_value_int)
        DO UPDATE SET
            last_seen_at = EXCLUDED.last_seen_at,
            trust_tier_rank = GREATEST(canonical_external_id.trust_tier_rank, EXCLUDED.trust_tier_rank);
    END IF;

    IF tvdb_id_value IS NOT NULL THEN
        INSERT INTO canonical_external_id (
            canonical_torrent_id,
            id_type,
            id_value_int,
            trust_tier_rank,
            first_seen_at,
            last_seen_at,
            source_canonical_torrent_source_id
        )
        VALUES (
            canonical_id,
            'tvdb',
            tvdb_id_value,
            instance_trust_rank,
            observed_at_value,
            observed_at_value,
            source_id
        )
        ON CONFLICT (canonical_torrent_id, id_type, id_value_int)
        DO UPDATE SET
            last_seen_at = EXCLUDED.last_seen_at,
            trust_tier_rank = GREATEST(canonical_external_id.trust_tier_rank, EXCLUDED.trust_tier_rank);
    END IF;

    SELECT id_value_text
    INTO best_imdb_id
    FROM canonical_external_id cei
    LEFT JOIN canonical_torrent_source_base_score bs
        ON bs.canonical_torrent_id = canonical_id
       AND bs.canonical_torrent_source_id = cei.source_canonical_torrent_source_id
    WHERE cei.canonical_torrent_id = canonical_id
      AND cei.id_type = 'imdb'
    ORDER BY cei.trust_tier_rank DESC,
             COALESCE(bs.score_total_base, 0) DESC,
             cei.last_seen_at DESC,
             cei.canonical_external_id_id ASC
    LIMIT 1;

    IF best_imdb_id IS NOT NULL THEN
        UPDATE canonical_torrent
        SET imdb_id = best_imdb_id,
            updated_at = now()
        WHERE canonical_torrent_id = canonical_id;
    END IF;

    SELECT id_value_int
    INTO best_tmdb_id
    FROM canonical_external_id cei
    LEFT JOIN canonical_torrent_source_base_score bs
        ON bs.canonical_torrent_id = canonical_id
       AND bs.canonical_torrent_source_id = cei.source_canonical_torrent_source_id
    WHERE cei.canonical_torrent_id = canonical_id
      AND cei.id_type = 'tmdb'
    ORDER BY cei.trust_tier_rank DESC,
             COALESCE(bs.score_total_base, 0) DESC,
             cei.last_seen_at DESC,
             cei.canonical_external_id_id ASC
    LIMIT 1;

    IF best_tmdb_id IS NOT NULL THEN
        UPDATE canonical_torrent
        SET tmdb_id = best_tmdb_id,
            updated_at = now()
        WHERE canonical_torrent_id = canonical_id;
    END IF;

    SELECT id_value_int
    INTO best_tvdb_id
    FROM canonical_external_id cei
    LEFT JOIN canonical_torrent_source_base_score bs
        ON bs.canonical_torrent_id = canonical_id
       AND bs.canonical_torrent_source_id = cei.source_canonical_torrent_source_id
    WHERE cei.canonical_torrent_id = canonical_id
      AND cei.id_type = 'tvdb'
    ORDER BY cei.trust_tier_rank DESC,
             COALESCE(bs.score_total_base, 0) DESC,
             cei.last_seen_at DESC,
             cei.canonical_external_id_id ASC
    LIMIT 1;

    IF best_tvdb_id IS NOT NULL THEN
        UPDATE canonical_torrent
        SET tvdb_id = best_tvdb_id,
            updated_at = now()
        WHERE canonical_torrent_id = canonical_id;
    END IF;

    IF identity_strategy_value <> 'title_size_fallback' THEN
        IF size_bytes_input IS NOT NULL AND size_bytes_input > 0 THEN
            IF request_effective_domain_id IS NOT NULL THEN
                SELECT media_domain_key::TEXT
                INTO media_domain_key_value
                FROM media_domain
                WHERE media_domain_id = request_effective_domain_id;
            ELSE
                SELECT array_agg(DISTINCT media_domain_id)
                INTO instance_domain_ids
                FROM indexer_instance_media_domain
                WHERE indexer_instance_id = instance_id;

                IF instance_domain_ids IS NOT NULL
                    AND array_length(instance_domain_ids, 1) = 1 THEN
                    SELECT media_domain_key::TEXT
                    INTO media_domain_key_value
                    FROM media_domain
                    WHERE media_domain_id = instance_domain_ids[1];
                ELSE
                    media_domain_key_value := NULL;
                END IF;
            END IF;

            IF size_bytes_input <= size_cutoff
                OR media_domain_key_value IN ('ebooks', 'audiobooks', 'software') THEN
                size_sample_allowed := TRUE;
            END IF;
        END IF;

        IF size_sample_allowed THEN
            INSERT INTO canonical_size_sample (
                canonical_torrent_id,
                observed_at,
                size_bytes
            )
            VALUES (
                canonical_id,
                observed_at_value,
                size_bytes_input
            )
            ON CONFLICT DO NOTHING;

            DELETE FROM canonical_size_sample
            WHERE canonical_torrent_id = canonical_id
              AND canonical_size_sample_id IN (
                  SELECT canonical_size_sample_id
                  FROM canonical_size_sample
                  WHERE canonical_torrent_id = canonical_id
                  ORDER BY observed_at DESC
                  OFFSET 25
              );

            SELECT COUNT(*), percentile_cont(0.5) WITHIN GROUP (ORDER BY size_bytes),
                   MIN(size_bytes), MAX(size_bytes)
            INTO sample_count, sample_median, sample_min, sample_max
            FROM canonical_size_sample
            WHERE canonical_torrent_id = canonical_id;

            IF sample_count IS NOT NULL AND sample_count > 0 THEN
                SELECT size_bytes
                INTO sample_first
                FROM canonical_size_sample
                WHERE canonical_torrent_id = canonical_id
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
                    canonical_id,
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

                UPDATE canonical_torrent
                SET size_bytes = CASE
                        WHEN sample_count >= 3 THEN sample_median::BIGINT
                        ELSE COALESCE(sample_first, sample_min)
                    END,
                    updated_at = now()
                WHERE canonical_torrent_id = canonical_id;
            END IF;
        END IF;
    ELSIF identity_strategy_value = 'title_size_fallback' AND size_bytes_input IS NOT NULL THEN
        UPDATE canonical_torrent
        SET size_bytes = COALESCE(size_bytes, size_bytes_input),
            updated_at = now()
        WHERE canonical_torrent_id = canonical_id;
    END IF;

    IF NOT dropped_canonical AND NOT dropped_source THEN
        INSERT INTO search_request_canonical (
            search_request_id,
            canonical_torrent_id
        )
        VALUES (
            request_id,
            canonical_id
        )
        ON CONFLICT (search_request_id, canonical_torrent_id) DO NOTHING
        RETURNING search_request_canonical_id INTO canonical_link_id;

        IF canonical_link_id IS NOT NULL THEN
            SELECT search_page_id, page_number
            INTO page_id, page_number_value
            FROM search_page
            WHERE search_request_id = request_id
              AND sealed_at IS NULL
            ORDER BY page_number DESC
            LIMIT 1;

            IF page_id IS NULL THEN
                page_number_value := 1;
                INSERT INTO search_page (search_request_id, page_number)
                VALUES (request_id, page_number_value)
                RETURNING search_page_id INTO page_id;
            END IF;

            SELECT COUNT(*)
            INTO page_item_count
            FROM search_page_item
            WHERE search_page_id = page_id;

            IF page_item_count >= request_page_size THEN
                UPDATE search_page
                SET sealed_at = now()
                WHERE search_page_id = page_id;

                page_number_value := page_number_value + 1;
                INSERT INTO search_page (search_request_id, page_number)
                VALUES (request_id, page_number_value)
                RETURNING search_page_id INTO page_id;

                page_item_count := 0;
            END IF;

            INSERT INTO search_page_item (
                search_page_id,
                search_request_canonical_id,
                position
            )
            VALUES (
                page_id,
                canonical_link_id,
                page_item_count + 1
            );
        END IF;
    END IF;

    IF request_snapshot_id IS NOT NULL THEN
        INSERT INTO search_filter_decision (
            search_request_id,
            policy_rule_public_id,
            policy_snapshot_id,
            observation_id,
            canonical_torrent_id,
            canonical_torrent_source_id,
            decision,
            decided_at
        )
        SELECT request_id,
               policy_rule_public_id,
               request_snapshot_id,
               observation_id_value,
               canonical_id,
               source_id,
               action::decision_type,
               now()
        FROM tmp_policy_matches;

        IF require_title_rule IS NOT NULL AND require_title_matched IS NOT TRUE THEN
            INSERT INTO search_filter_decision (
                search_request_id,
                policy_rule_public_id,
                policy_snapshot_id,
                observation_id,
                canonical_torrent_id,
                canonical_torrent_source_id,
                decision,
                decided_at
            )
            VALUES (
                request_id,
                require_title_rule,
                request_snapshot_id,
                observation_id_value,
                canonical_id,
                source_id,
                'drop_canonical',
                now()
            );
        END IF;

        IF require_release_group_rule IS NOT NULL AND require_release_group_matched IS NOT TRUE THEN
            INSERT INTO search_filter_decision (
                search_request_id,
                policy_rule_public_id,
                policy_snapshot_id,
                observation_id,
                canonical_torrent_id,
                canonical_torrent_source_id,
                decision,
                decided_at
            )
            VALUES (
                request_id,
                require_release_group_rule,
                request_snapshot_id,
                observation_id_value,
                canonical_id,
                source_id,
                'drop_canonical',
                now()
            );
        END IF;

        IF require_indexer_rule IS NOT NULL AND require_indexer_matched IS NOT TRUE THEN
            INSERT INTO search_filter_decision (
                search_request_id,
                policy_rule_public_id,
                policy_snapshot_id,
                observation_id,
                canonical_torrent_id,
                canonical_torrent_source_id,
                decision,
                decided_at
            )
            VALUES (
                request_id,
                require_indexer_rule,
                request_snapshot_id,
                observation_id_value,
                canonical_id,
                source_id,
                'drop_source',
                now()
            );
        END IF;

        IF require_domain_rule IS NOT NULL AND require_domain_matched IS NOT TRUE THEN
            INSERT INTO search_filter_decision (
                search_request_id,
                policy_rule_public_id,
                policy_snapshot_id,
                observation_id,
                canonical_torrent_id,
                canonical_torrent_source_id,
                decision,
                decided_at
            )
            VALUES (
                request_id,
                require_domain_rule,
                request_snapshot_id,
                observation_id_value,
                canonical_id,
                source_id,
                'drop_source',
                now()
            );
        END IF;

        IF require_trust_rule IS NOT NULL AND require_trust_matched IS NOT TRUE THEN
            INSERT INTO search_filter_decision (
                search_request_id,
                policy_rule_public_id,
                policy_snapshot_id,
                observation_id,
                canonical_torrent_id,
                canonical_torrent_source_id,
                decision,
                decided_at
            )
            VALUES (
                request_id,
                require_trust_rule,
                request_snapshot_id,
                observation_id_value,
                canonical_id,
                source_id,
                'drop_source',
                now()
            );
        END IF;
    END IF;

    canonical_torrent_public_id := canonical_public_id;
    canonical_torrent_source_public_id := source_public_id;
    observation_created := observation_inserted;
    durable_source_created := source_inserted;
    canonical_changed := canonical_inserted;
    RETURN NEXT;
END;
$$;

CREATE OR REPLACE FUNCTION search_result_ingest(
    search_request_public_id_input UUID,
    indexer_instance_public_id_input UUID,
    source_guid_input VARCHAR,
    details_url_input VARCHAR,
    download_url_input VARCHAR,
    magnet_uri_input VARCHAR,
    title_raw_input VARCHAR,
    size_bytes_input BIGINT,
    infohash_v1_input CHAR(40),
    infohash_v2_input CHAR(64),
    magnet_hash_input CHAR(64),
    seeders_input INTEGER,
    leechers_input INTEGER,
    published_at_input TIMESTAMPTZ,
    uploader_input VARCHAR,
    observed_at_input TIMESTAMPTZ,
    attr_keys_input observation_attr_key[],
    attr_types_input attr_value_type[],
    attr_value_text_input VARCHAR[],
    attr_value_int_input INTEGER[],
    attr_value_bigint_input BIGINT[],
    attr_value_numeric_input NUMERIC(12, 4)[],
    attr_value_bool_input BOOLEAN[],
    attr_value_uuid_input UUID[]
)
RETURNS TABLE(
    canonical_torrent_public_id UUID,
    canonical_torrent_source_public_id UUID,
    observation_created BOOLEAN,
    durable_source_created BOOLEAN,
    canonical_changed BOOLEAN
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT * FROM search_result_ingest_v1(
        search_request_public_id_input,
        indexer_instance_public_id_input,
        source_guid_input,
        details_url_input,
        download_url_input,
        magnet_uri_input,
        title_raw_input,
        size_bytes_input,
        infohash_v1_input,
        infohash_v2_input,
        magnet_hash_input,
        seeders_input,
        leechers_input,
        published_at_input,
        uploader_input,
        observed_at_input,
        attr_keys_input,
        attr_types_input,
        attr_value_text_input,
        attr_value_int_input,
        attr_value_bigint_input,
        attr_value_numeric_input,
        attr_value_bool_input,
        attr_value_uuid_input
    );
END;
$$;
