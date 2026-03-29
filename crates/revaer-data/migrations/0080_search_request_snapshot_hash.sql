-- Fix ambiguous snapshot hash lookup in search_request_create_v1.

CREATE OR REPLACE FUNCTION search_request_create_v1(
    actor_user_public_id UUID,
    query_text_input VARCHAR,
    query_type_input query_type,
    torznab_mode_input torznab_mode,
    requested_media_domain_key_input VARCHAR,
    page_size_input INTEGER,
    search_profile_public_id_input UUID,
    request_policy_set_public_id_input UUID,
    season_number_input INTEGER,
    episode_number_input INTEGER,
    identifier_types_input identifier_type[],
    identifier_values_input TEXT[],
    torznab_cat_ids_input INTEGER[]
)
RETURNS TABLE(search_request_public_id UUID, request_policy_set_public_id UUID)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to create search request';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    system_user_id BIGINT := 0;
    resolved_query_text TEXT;
    query_text_trimmed TEXT;
    resolved_query_type query_type;
    resolved_torznab_mode torznab_mode;
    resolved_page_size INTEGER;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    profile_page_size INTEGER;
    profile_default_media_domain_id BIGINT;
    normalized_domain_key VARCHAR(128);
    requested_media_domain_id BIGINT;
    request_policy_set_id BIGINT;
    request_policy_set_public_id_value UUID;
    request_policy_set_user_id BIGINT;
    request_policy_set_deleted_at TIMESTAMPTZ;
    request_policy_set_scope policy_scope;
    request_policy_set_enabled BOOLEAN;
    auto_policy_set_created BOOLEAN := FALSE;
    global_policy_set_id BIGINT;
    global_policy_set_public_id UUID;
    user_policy_set_id BIGINT;
    user_policy_set_public_id UUID;
    profile_policy_set_ids BIGINT[];
    profile_policy_set_public_ids UUID[];
    profile_policy_set_csv TEXT;
    global_policy_set_csv TEXT;
    user_policy_set_csv TEXT;
    request_policy_set_csv TEXT;
    scope_bitmap INTEGER := 0;
    ordered_rule_public_ids UUID[];
    rule_public_ids_csv TEXT;
    snapshot_hash_value TEXT;
    snapshot_id BIGINT;
    snapshot_inserted BOOLEAN := FALSE;
    excluded_disabled_count INTEGER := 0;
    excluded_expired_count INTEGER := 0;
    canonical_string TEXT;
    explicit_identifier_count INTEGER := 0;
    identifier_type_value identifier_type;
    identifier_raw_value TEXT;
    identifier_normalized_value TEXT;
    imdb_pattern TEXT := '(?i)(?:^|[^a-z0-9])(tt[0-9]{7,9})(?:$|[^a-z0-9])';
    tmdb_pattern TEXT := '(?i)(?:^|[^a-z0-9])(tmdb[:\\s]*([0-9]{1,10}))(?:$|[^a-z0-9])';
    tvdb_pattern TEXT := '(?i)(?:^|[^a-z0-9])(tvdb[:\\s]*([0-9]{1,10}))(?:$|[^a-z0-9])';
    imdb_count INTEGER := 0;
    tmdb_count INTEGER := 0;
    tvdb_count INTEGER := 0;
    input_cat_count INTEGER := 0;
    requested_cat_ids BIGINT[];
    requested_cat_count INTEGER := 0;
    effective_cat_ids BIGINT[];
    effective_cat_count INTEGER := 0;
    requested_has_8000 BOOLEAN := FALSE;
    profile_allow_domain_ids BIGINT[];
    profile_allow_domain_count INTEGER := 0;
    category_domain_ids BIGINT[];
    policy_domain_ids BIGINT[];
    allowed_domain_ids BIGINT[];
    allowed_domain_count INTEGER := 0;
    constraint_count INTEGER := 0;
    effective_media_domain_id BIGINT;
    has_policy_indexer_allow BOOLEAN := FALSE;
    policy_allowed_indexer_ids BIGINT[];
    policy_allowed_indexer_count INTEGER := 0;
    profile_has_indexer_allow BOOLEAN := FALSE;
    profile_has_indexer_block BOOLEAN := FALSE;
    profile_has_tag_allow BOOLEAN := FALSE;
    profile_has_tag_block BOOLEAN := FALSE;
    runnable_indexer_ids BIGINT[];
    runnable_indexer_count INTEGER := 0;
    search_request_id BIGINT;
    new_search_request_public_id UUID;
    status_value search_status;
    finished_at_value TIMESTAMPTZ;
BEGIN
    IF actor_user_public_id IS NULL THEN
        IF torznab_mode_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_missing';
        END IF;
    ELSE
        SELECT user_id, role
        INTO actor_user_id, actor_role
        FROM app_user
        WHERE user_public_id = actor_user_public_id;

        IF actor_user_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_not_found';
        END IF;
    END IF;

    IF query_text_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'query_text_missing';
    END IF;

    resolved_query_text := query_text_input;
    query_text_trimmed := trim(query_text_input);

    IF char_length(resolved_query_text) > 512 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'query_text_too_long';
    END IF;

    IF identifier_types_input IS NULL AND identifier_values_input IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'identifier_input_invalid';
    END IF;

    IF identifier_types_input IS NOT NULL AND identifier_values_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'identifier_input_invalid';
    END IF;

    explicit_identifier_count := COALESCE(array_length(identifier_types_input, 1), 0);
    IF explicit_identifier_count <> COALESCE(array_length(identifier_values_input, 1), 0) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'identifier_input_invalid';
    END IF;

    IF explicit_identifier_count > 1 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'invalid_identifier_combo';
    END IF;

    IF explicit_identifier_count = 1 THEN
        identifier_type_value := identifier_types_input[1];
        identifier_raw_value := identifier_values_input[1];

        IF identifier_type_value IS NULL OR identifier_raw_value IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_query';
        END IF;

        identifier_raw_value := trim(identifier_raw_value);
        IF identifier_raw_value = '' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_query';
        END IF;

        IF identifier_type_value = 'imdb' THEN
            identifier_raw_value := lower(identifier_raw_value);
            IF identifier_raw_value ~ '^tt[0-9]{7,9}$' THEN
                identifier_normalized_value := identifier_raw_value;
            ELSIF identifier_raw_value ~ '^[0-9]{7,9}$' THEN
                identifier_normalized_value := 'tt' || identifier_raw_value;
            ELSE
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'invalid_query';
            END IF;
        ELSIF identifier_type_value IN ('tmdb', 'tvdb') THEN
            IF identifier_raw_value !~ '^[0-9]{1,10}$' THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'invalid_query';
            END IF;
            identifier_normalized_value := identifier_raw_value;
        ELSE
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_identifier_combo';
        END IF;
    ELSE
        SELECT count(*) INTO imdb_count
        FROM regexp_matches(resolved_query_text, imdb_pattern, 'g');
        SELECT count(*) INTO tmdb_count
        FROM regexp_matches(resolved_query_text, tmdb_pattern, 'g');
        SELECT count(*) INTO tvdb_count
        FROM regexp_matches(resolved_query_text, tvdb_pattern, 'g');

        IF (imdb_count > 0 AND tmdb_count > 0)
            OR (imdb_count > 0 AND tvdb_count > 0)
            OR (tmdb_count > 0 AND tvdb_count > 0) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_identifier_combo';
        END IF;

        IF imdb_count > 1 OR tmdb_count > 1 OR tvdb_count > 1 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_identifier_combo';
        END IF;

        IF imdb_count = 1 THEN
            SELECT lower((regexp_matches(resolved_query_text, imdb_pattern, 'g'))[1])
            INTO identifier_raw_value;
            identifier_type_value := 'imdb';
            identifier_normalized_value := identifier_raw_value;
        ELSIF tmdb_count = 1 THEN
            SELECT lower((regexp_matches(resolved_query_text, tmdb_pattern, 'g'))[1]),
                   (regexp_matches(resolved_query_text, tmdb_pattern, 'g'))[2]
            INTO identifier_raw_value, identifier_normalized_value;
            identifier_type_value := 'tmdb';
            identifier_raw_value := trim(identifier_raw_value);
            identifier_normalized_value := trim(identifier_normalized_value);
        ELSIF tvdb_count = 1 THEN
            SELECT lower((regexp_matches(resolved_query_text, tvdb_pattern, 'g'))[1]),
                   (regexp_matches(resolved_query_text, tvdb_pattern, 'g'))[2]
            INTO identifier_raw_value, identifier_normalized_value;
            identifier_type_value := 'tvdb';
            identifier_raw_value := trim(identifier_raw_value);
            identifier_normalized_value := trim(identifier_normalized_value);
        END IF;
    END IF;

    IF (query_text_trimmed = '' AND identifier_type_value IS NULL) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'invalid_query';
    END IF;

    IF season_number_input IS NOT NULL AND season_number_input < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'invalid_season_episode_combo';
    END IF;

    IF episode_number_input IS NOT NULL AND episode_number_input < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'invalid_season_episode_combo';
    END IF;

    IF torznab_mode_input IS NOT NULL THEN
        IF actor_user_public_id IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_torznab_mode';
        END IF;

        resolved_torznab_mode := torznab_mode_input;

        IF resolved_torznab_mode IN ('generic', 'movie') THEN
            IF season_number_input IS NOT NULL OR episode_number_input IS NOT NULL THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'invalid_season_episode_combo';
            END IF;
        ELSIF resolved_torznab_mode = 'tv' THEN
            IF episode_number_input IS NOT NULL AND season_number_input IS NULL THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'invalid_season_episode_combo';
            END IF;
            IF season_number_input IS NOT NULL
                AND query_text_trimmed = ''
                AND identifier_type_value IS NULL THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'invalid_query';
            END IF;
        END IF;

        IF resolved_torznab_mode = 'movie' AND identifier_type_value = 'tvdb' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_identifier_combo';
        END IF;

        IF identifier_type_value IS NOT NULL THEN
            resolved_query_type := identifier_type_value;
        ELSE
            resolved_query_type := 'free_text';
        END IF;
    ELSE
        resolved_torznab_mode := NULL;

        IF query_type_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'query_type_missing';
        END IF;

        IF identifier_type_value IS NOT NULL THEN
            resolved_query_type := identifier_type_value;
            IF query_type_input IN ('imdb', 'tmdb', 'tvdb')
                AND query_type_input <> identifier_type_value THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'invalid_identifier_mismatch';
            END IF;
        ELSE
            resolved_query_type := query_type_input;
        END IF;

        IF query_type_input IN ('imdb', 'tmdb', 'tvdb')
            AND identifier_type_value IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_query';
        END IF;

        IF resolved_query_type = 'season_episode' THEN
            IF season_number_input IS NULL OR episode_number_input IS NULL THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'invalid_season_episode_combo';
            END IF;
            IF query_text_trimmed = '' AND identifier_type_value IS NULL THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'invalid_query';
            END IF;
        ELSE
            IF season_number_input IS NOT NULL OR episode_number_input IS NOT NULL THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'invalid_season_episode_combo';
            END IF;
        END IF;
    END IF;

    IF search_profile_public_id_input IS NOT NULL THEN
        SELECT search_profile_id,
               user_id,
               deleted_at,
               page_size,
               default_media_domain_id
        INTO profile_id,
             profile_user_id,
             profile_deleted_at,
             profile_page_size,
             profile_default_media_domain_id
        FROM search_profile
        WHERE search_profile_public_id = search_profile_public_id_input;

        IF profile_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'search_profile_not_found';
        END IF;

        IF profile_deleted_at IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'search_profile_deleted';
        END IF;

        IF profile_user_id IS NOT NULL THEN
            IF actor_user_id IS NULL THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'actor_unauthorized';
            END IF;

            IF profile_user_id <> actor_user_id AND actor_role NOT IN ('owner', 'admin') THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'actor_unauthorized';
            END IF;
        END IF;
    END IF;

    resolved_page_size := page_size_input;
    IF resolved_page_size IS NULL THEN
        resolved_page_size := profile_page_size;
    END IF;

    IF resolved_page_size IS NULL THEN
        SELECT default_page_size
        INTO resolved_page_size
        FROM deployment_config
        ORDER BY deployment_config_id DESC
        LIMIT 1;
    END IF;

    IF resolved_page_size IS NULL THEN
        resolved_page_size := 50;
    END IF;

    IF resolved_page_size < 10 THEN
        resolved_page_size := 10;
    ELSIF resolved_page_size > 200 THEN
        resolved_page_size := 200;
    END IF;

    IF requested_media_domain_key_input IS NOT NULL THEN
        normalized_domain_key := lower(trim(requested_media_domain_key_input));
        IF normalized_domain_key = ''
            OR normalized_domain_key <> requested_media_domain_key_input THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'media_domain_key_invalid';
        END IF;

        SELECT media_domain_id
        INTO requested_media_domain_id
        FROM media_domain
        WHERE media_domain_key::TEXT = normalized_domain_key;

        IF requested_media_domain_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'unknown_key';
        END IF;
    ELSE
        requested_media_domain_id := profile_default_media_domain_id;
    END IF;

    IF request_policy_set_public_id_input IS NOT NULL THEN
        SELECT policy_set_id,
               policy_set_public_id,
               user_id,
               scope,
               is_enabled,
               deleted_at
        INTO request_policy_set_id,
             request_policy_set_public_id_value,
             request_policy_set_user_id,
             request_policy_set_scope,
             request_policy_set_enabled,
             request_policy_set_deleted_at
        FROM policy_set
        WHERE policy_set_public_id = request_policy_set_public_id_input;

        IF request_policy_set_id IS NULL
            OR request_policy_set_deleted_at IS NOT NULL
            OR request_policy_set_scope <> 'request'
            OR request_policy_set_enabled IS DISTINCT FROM TRUE THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_request_policy_set';
        END IF;

        IF actor_user_id IS NULL THEN
            IF request_policy_set_user_id IS NOT NULL THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'invalid_request_policy_set';
            END IF;
        ELSE
            IF request_policy_set_user_id IS DISTINCT FROM actor_user_id THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'invalid_request_policy_set';
            END IF;
        END IF;
    ELSE
        request_policy_set_public_id_value := gen_random_uuid();

        INSERT INTO policy_set (
            policy_set_public_id,
            user_id,
            display_name,
            scope,
            is_enabled,
            sort_order,
            is_auto_created,
            created_for_search_request_id,
            created_by_user_id,
            updated_by_user_id
        )
        VALUES (
            request_policy_set_public_id_value,
            NULL,
            'Auto-created request policy set',
            'request',
            TRUE,
            1000,
            TRUE,
            NULL,
            COALESCE(actor_user_id, system_user_id),
            COALESCE(actor_user_id, system_user_id)
        )
        RETURNING policy_set_id INTO request_policy_set_id;

        auto_policy_set_created := TRUE;
    END IF;

    SELECT policy_set_id, policy_set_public_id
    INTO global_policy_set_id, global_policy_set_public_id
    FROM policy_set
    WHERE scope = 'global'
      AND is_enabled = TRUE
      AND deleted_at IS NULL
    ORDER BY sort_order, created_at, policy_set_public_id
    LIMIT 1;

    IF actor_user_id IS NOT NULL THEN
        SELECT policy_set_id, policy_set_public_id
        INTO user_policy_set_id, user_policy_set_public_id
        FROM policy_set
        WHERE scope = 'user'
          AND user_id = actor_user_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        ORDER BY sort_order, created_at, policy_set_public_id
        LIMIT 1;
    END IF;

    IF profile_id IS NOT NULL THEN
        SELECT array_agg(ps.policy_set_id ORDER BY ps.sort_order, ps.created_at, ps.policy_set_public_id),
               array_agg(ps.policy_set_public_id ORDER BY ps.sort_order, ps.created_at, ps.policy_set_public_id)
        INTO profile_policy_set_ids, profile_policy_set_public_ids
        FROM policy_set ps
        JOIN search_profile_policy_set spps
            ON spps.policy_set_id = ps.policy_set_id
        WHERE spps.search_profile_id = profile_id
          AND ps.is_enabled = TRUE
          AND ps.deleted_at IS NULL;
    END IF;

    IF profile_policy_set_ids IS NULL THEN
        profile_policy_set_ids := ARRAY[]::BIGINT[];
    END IF;

    IF profile_policy_set_public_ids IS NULL THEN
        profile_policy_set_public_ids := ARRAY[]::UUID[];
    END IF;

    IF global_policy_set_id IS NOT NULL THEN
        scope_bitmap := scope_bitmap + 1;
    END IF;

    IF user_policy_set_id IS NOT NULL THEN
        scope_bitmap := scope_bitmap + 2;
    END IF;

    IF array_length(profile_policy_set_ids, 1) IS NOT NULL THEN
        scope_bitmap := scope_bitmap + 4;
    END IF;

    IF request_policy_set_id IS NOT NULL THEN
        scope_bitmap := scope_bitmap + 8;
    END IF;

    IF global_policy_set_public_id IS NULL THEN
        global_policy_set_csv := '-';
    ELSE
        global_policy_set_csv := global_policy_set_public_id::TEXT;
    END IF;

    IF user_policy_set_public_id IS NULL THEN
        user_policy_set_csv := '-';
    ELSE
        user_policy_set_csv := user_policy_set_public_id::TEXT;
    END IF;

    IF array_length(profile_policy_set_public_ids, 1) IS NULL THEN
        profile_policy_set_csv := '-';
    ELSE
        profile_policy_set_csv := array_to_string(profile_policy_set_public_ids, ',');
    END IF;

    request_policy_set_csv := request_policy_set_public_id_value::TEXT;

    WITH scoped_policy_sets AS (
        SELECT policy_set_id, policy_set_public_id, sort_order, created_at, 1 AS precedence_rank
        FROM policy_set
        WHERE policy_set_id = request_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id, policy_set_public_id, sort_order, created_at, 2
        FROM policy_set
        WHERE policy_set_id = ANY(profile_policy_set_ids)
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id, policy_set_public_id, sort_order, created_at, 3
        FROM policy_set
        WHERE policy_set_id = user_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id, policy_set_public_id, sort_order, created_at, 4
        FROM policy_set
        WHERE policy_set_id = global_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
    )
    SELECT array_agg(
        pr.policy_rule_public_id
        ORDER BY
            scoped_policy_sets.precedence_rank,
            scoped_policy_sets.sort_order,
            scoped_policy_sets.created_at,
            scoped_policy_sets.policy_set_public_id,
            pr.sort_order,
            pr.policy_rule_public_id
    )
    INTO ordered_rule_public_ids
    FROM policy_rule pr
    JOIN scoped_policy_sets
        ON scoped_policy_sets.policy_set_id = pr.policy_set_id
    WHERE pr.is_disabled = FALSE
      AND (pr.expires_at IS NULL OR pr.expires_at >= now());

    WITH scoped_policy_sets AS (
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = request_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = ANY(profile_policy_set_ids)
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = user_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = global_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
    )
    SELECT count(*)
    INTO excluded_disabled_count
    FROM policy_rule pr
    JOIN scoped_policy_sets
        ON scoped_policy_sets.policy_set_id = pr.policy_set_id
    WHERE pr.is_disabled = TRUE;

    WITH scoped_policy_sets AS (
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = request_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = ANY(profile_policy_set_ids)
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = user_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = global_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
    )
    SELECT count(*)
    INTO excluded_expired_count
    FROM policy_rule pr
    JOIN scoped_policy_sets
        ON scoped_policy_sets.policy_set_id = pr.policy_set_id
    WHERE pr.is_disabled = FALSE
      AND pr.expires_at IS NOT NULL
      AND pr.expires_at < now();

    IF ordered_rule_public_ids IS NULL
        OR array_length(ordered_rule_public_ids, 1) IS NULL THEN
        rule_public_ids_csv := '-';
    ELSE
        rule_public_ids_csv := array_to_string(ordered_rule_public_ids, ',');
    END IF;

    canonical_string := scope_bitmap::TEXT
        || '|g=' || global_policy_set_csv
        || '|u=' || user_policy_set_csv
        || '|p=' || profile_policy_set_csv
        || '|r=' || request_policy_set_csv
        || '|rules=' || rule_public_ids_csv;

    snapshot_hash_value := lower(encode(digest(canonical_string, 'sha256'), 'hex'));

    INSERT INTO policy_snapshot (
        snapshot_hash,
        ref_count,
        excluded_disabled_count,
        excluded_expired_count
    )
    VALUES (
        snapshot_hash_value,
        0,
        excluded_disabled_count,
        excluded_expired_count
    )
    ON CONFLICT (snapshot_hash) DO NOTHING
    RETURNING policy_snapshot_id INTO snapshot_id;

    IF snapshot_id IS NULL THEN
        SELECT policy_snapshot_id
        INTO snapshot_id
        FROM policy_snapshot
        WHERE policy_snapshot.snapshot_hash = snapshot_hash_value;
    ELSE
        snapshot_inserted := TRUE;
    END IF;

    IF snapshot_inserted
        AND ordered_rule_public_ids IS NOT NULL
        AND array_length(ordered_rule_public_ids, 1) IS NOT NULL THEN
        INSERT INTO policy_snapshot_rule (
            policy_snapshot_id,
            policy_rule_public_id,
            rule_order
        )
        SELECT snapshot_id, value, ordinality
        FROM unnest(ordered_rule_public_ids) WITH ORDINALITY AS t(value, ordinality);
    END IF;

    WITH scoped_policy_sets AS (
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = request_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = ANY(profile_policy_set_ids)
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = user_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = global_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
    ), policy_domain_keys AS (
        SELECT pr.match_value_text AS value_text
        FROM policy_rule pr
        JOIN scoped_policy_sets
            ON scoped_policy_sets.policy_set_id = pr.policy_set_id
        WHERE pr.rule_type = 'require_media_domain'
          AND pr.action = 'require'
          AND pr.is_disabled = FALSE
          AND (pr.expires_at IS NULL OR pr.expires_at >= now())
          AND pr.match_operator = 'eq'
          AND pr.match_value_text IS NOT NULL
        UNION
        SELECT vsi.value_text
        FROM policy_rule pr
        JOIN policy_rule_value_set_item vsi
            ON vsi.value_set_id = pr.value_set_id
        JOIN scoped_policy_sets
            ON scoped_policy_sets.policy_set_id = pr.policy_set_id
        WHERE pr.rule_type = 'require_media_domain'
          AND pr.action = 'require'
          AND pr.is_disabled = FALSE
          AND (pr.expires_at IS NULL OR pr.expires_at >= now())
          AND pr.match_operator = 'in_set'
          AND vsi.value_text IS NOT NULL
    )
    SELECT array_agg(DISTINCT md.media_domain_id)
    INTO policy_domain_ids
    FROM media_domain md
    JOIN policy_domain_keys pdk
        ON md.media_domain_key::TEXT = pdk.value_text;

    WITH scoped_policy_sets AS (
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = request_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = ANY(profile_policy_set_ids)
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = user_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
        UNION ALL
        SELECT policy_set_id
        FROM policy_set
        WHERE policy_set_id = global_policy_set_id
          AND is_enabled = TRUE
          AND deleted_at IS NULL
    )
    SELECT EXISTS (
        SELECT 1
        FROM policy_rule pr
        JOIN scoped_policy_sets
            ON scoped_policy_sets.policy_set_id = pr.policy_set_id
        WHERE pr.rule_type = 'allow_indexer_instance'
          AND pr.action = 'require'
          AND pr.is_disabled = FALSE
          AND (pr.expires_at IS NULL OR pr.expires_at >= now())
    )
    INTO has_policy_indexer_allow;

    IF has_policy_indexer_allow THEN
        WITH scoped_policy_sets AS (
            SELECT policy_set_id
            FROM policy_set
            WHERE policy_set_id = request_policy_set_id
              AND is_enabled = TRUE
              AND deleted_at IS NULL
            UNION ALL
            SELECT policy_set_id
            FROM policy_set
            WHERE policy_set_id = ANY(profile_policy_set_ids)
              AND is_enabled = TRUE
              AND deleted_at IS NULL
            UNION ALL
            SELECT policy_set_id
            FROM policy_set
            WHERE policy_set_id = user_policy_set_id
              AND is_enabled = TRUE
              AND deleted_at IS NULL
            UNION ALL
            SELECT policy_set_id
            FROM policy_set
            WHERE policy_set_id = global_policy_set_id
              AND is_enabled = TRUE
              AND deleted_at IS NULL
        ), allowed_indexer_public_ids AS (
            SELECT pr.match_value_uuid AS value_uuid
            FROM policy_rule pr
            JOIN scoped_policy_sets
                ON scoped_policy_sets.policy_set_id = pr.policy_set_id
            WHERE pr.rule_type = 'allow_indexer_instance'
              AND pr.action = 'require'
              AND pr.is_disabled = FALSE
              AND (pr.expires_at IS NULL OR pr.expires_at >= now())
              AND pr.match_operator = 'eq'
              AND pr.match_value_uuid IS NOT NULL
            UNION
            SELECT vsi.value_uuid
            FROM policy_rule pr
            JOIN policy_rule_value_set_item vsi
                ON vsi.value_set_id = pr.value_set_id
            JOIN scoped_policy_sets
                ON scoped_policy_sets.policy_set_id = pr.policy_set_id
            WHERE pr.rule_type = 'allow_indexer_instance'
              AND pr.action = 'require'
              AND pr.is_disabled = FALSE
              AND (pr.expires_at IS NULL OR pr.expires_at >= now())
              AND pr.match_operator = 'in_set'
              AND vsi.value_uuid IS NOT NULL
        )
        SELECT array_agg(DISTINCT idx.indexer_instance_id)
        INTO policy_allowed_indexer_ids
        FROM indexer_instance idx
        JOIN allowed_indexer_public_ids aid
            ON idx.indexer_instance_public_id = aid.value_uuid
        WHERE idx.deleted_at IS NULL;
    END IF;

    IF policy_allowed_indexer_ids IS NULL THEN
        policy_allowed_indexer_ids := ARRAY[]::BIGINT[];
    END IF;
    policy_allowed_indexer_count := COALESCE(array_length(policy_allowed_indexer_ids, 1), 0);

    IF profile_id IS NOT NULL THEN
        SELECT array_agg(DISTINCT media_domain_id)
        INTO profile_allow_domain_ids
        FROM search_profile_media_domain
        WHERE search_profile_id = profile_id;

        profile_allow_domain_count := COALESCE(array_length(profile_allow_domain_ids, 1), 0);
        IF profile_allow_domain_count = 0 THEN
            profile_allow_domain_ids := NULL;
        END IF;

        SELECT EXISTS (
            SELECT 1 FROM search_profile_indexer_allow WHERE search_profile_id = profile_id
        ) INTO profile_has_indexer_allow;

        SELECT EXISTS (
            SELECT 1 FROM search_profile_indexer_block WHERE search_profile_id = profile_id
        ) INTO profile_has_indexer_block;

        SELECT EXISTS (
            SELECT 1 FROM search_profile_tag_allow WHERE search_profile_id = profile_id
        ) INTO profile_has_tag_allow;

        SELECT EXISTS (
            SELECT 1 FROM search_profile_tag_block WHERE search_profile_id = profile_id
        ) INTO profile_has_tag_block;
    END IF;

    IF torznab_cat_ids_input IS NOT NULL THEN
        SELECT count(*)
        INTO input_cat_count
        FROM unnest(torznab_cat_ids_input) AS value;
    END IF;

    IF input_cat_count > 0 THEN
        SELECT array_agg(DISTINCT torznab_category_id)
        INTO requested_cat_ids
        FROM torznab_category
        WHERE torznab_cat_id = ANY(torznab_cat_ids_input);
    ELSE
        requested_cat_ids := ARRAY[]::BIGINT[];
    END IF;

    IF requested_cat_ids IS NULL THEN
        requested_cat_ids := ARRAY[]::BIGINT[];
    END IF;

    requested_cat_count := COALESCE(array_length(requested_cat_ids, 1), 0);

    IF input_cat_count > 0 AND requested_cat_count = 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'invalid_category_filter';
    END IF;

    IF requested_cat_count > 0 THEN
        SELECT EXISTS (
            SELECT 1
            FROM torznab_category
            WHERE torznab_category_id = ANY(requested_cat_ids)
              AND torznab_cat_id = 8000
        ) INTO requested_has_8000;
    END IF;

    IF requested_cat_count = 0 THEN
        effective_cat_ids := ARRAY[]::BIGINT[];
    ELSIF requested_has_8000 THEN
        effective_cat_ids := requested_cat_ids;
    ELSE
        IF profile_allow_domain_ids IS NOT NULL THEN
            SELECT array_agg(DISTINCT mdtc.torznab_category_id)
            INTO effective_cat_ids
            FROM media_domain_to_torznab_category mdtc
            WHERE mdtc.media_domain_id = ANY(profile_allow_domain_ids)
              AND mdtc.torznab_category_id = ANY(requested_cat_ids);

            IF effective_cat_ids IS NULL THEN
                effective_cat_ids := ARRAY[]::BIGINT[];
            END IF;

            effective_cat_count := COALESCE(array_length(effective_cat_ids, 1), 0);
            IF effective_cat_count = 0 THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'invalid_category_filter';
            END IF;
        ELSE
            effective_cat_ids := requested_cat_ids;
        END IF;
    END IF;

    IF effective_cat_ids IS NULL THEN
        effective_cat_ids := ARRAY[]::BIGINT[];
    END IF;

    effective_cat_count := COALESCE(array_length(effective_cat_ids, 1), 0);

    IF requested_has_8000 OR effective_cat_count = 0 THEN
        category_domain_ids := NULL;
    ELSE
        SELECT array_agg(DISTINCT media_domain_id)
        INTO category_domain_ids
        FROM media_domain_to_torznab_category
        WHERE torznab_category_id = ANY(effective_cat_ids);
    END IF;

    IF requested_media_domain_id IS NOT NULL THEN
        allowed_domain_ids := ARRAY[requested_media_domain_id];
        constraint_count := constraint_count + 1;
    END IF;

    IF category_domain_ids IS NOT NULL
        AND array_length(category_domain_ids, 1) IS NOT NULL THEN
        constraint_count := constraint_count + 1;
        IF allowed_domain_ids IS NULL THEN
            allowed_domain_ids := category_domain_ids;
        ELSE
            SELECT array_agg(DISTINCT value)
            INTO allowed_domain_ids
            FROM (
                SELECT unnest(allowed_domain_ids) AS value
                INTERSECT
                SELECT unnest(category_domain_ids) AS value
            ) AS intersected;
        END IF;
    END IF;

    IF policy_domain_ids IS NOT NULL
        AND array_length(policy_domain_ids, 1) IS NOT NULL THEN
        constraint_count := constraint_count + 1;
        IF allowed_domain_ids IS NULL THEN
            allowed_domain_ids := policy_domain_ids;
        ELSE
            SELECT array_agg(DISTINCT value)
            INTO allowed_domain_ids
            FROM (
                SELECT unnest(allowed_domain_ids) AS value
                INTERSECT
                SELECT unnest(policy_domain_ids) AS value
            ) AS intersected;
        END IF;
    END IF;

    IF profile_allow_domain_ids IS NOT NULL
        AND array_length(profile_allow_domain_ids, 1) IS NOT NULL THEN
        constraint_count := constraint_count + 1;
        IF allowed_domain_ids IS NULL THEN
            allowed_domain_ids := profile_allow_domain_ids;
        ELSE
            SELECT array_agg(DISTINCT value)
            INTO allowed_domain_ids
            FROM (
                SELECT unnest(allowed_domain_ids) AS value
                INTERSECT
                SELECT unnest(profile_allow_domain_ids) AS value
            ) AS intersected;
        END IF;
    END IF;

    IF constraint_count = 0 THEN
        allowed_domain_ids := NULL;
    ELSE
        IF allowed_domain_ids IS NULL THEN
            allowed_domain_ids := ARRAY[]::BIGINT[];
        END IF;
        allowed_domain_count := COALESCE(array_length(allowed_domain_ids, 1), 0);
    END IF;

    IF constraint_count = 0 THEN
        effective_media_domain_id := NULL;
    ELSE
        IF allowed_domain_count = 1 THEN
            effective_media_domain_id := allowed_domain_ids[1];
        ELSE
            effective_media_domain_id := NULL;
        END IF;
    END IF;

    IF constraint_count > 0 AND allowed_domain_count = 0 THEN
        runnable_indexer_ids := ARRAY[]::BIGINT[];
    ELSIF has_policy_indexer_allow AND policy_allowed_indexer_count = 0 THEN
        runnable_indexer_ids := ARRAY[]::BIGINT[];
    ELSE
        SELECT array_agg(idx.indexer_instance_id ORDER BY idx.indexer_instance_id)
        INTO runnable_indexer_ids
        FROM indexer_instance idx
        WHERE idx.deleted_at IS NULL
          AND idx.is_enabled = TRUE
          AND (idx.migration_state IS NULL OR idx.migration_state = 'ready')
          AND idx.enable_interactive_search = TRUE
          AND (
              NOT profile_has_indexer_allow
              OR EXISTS (
                  SELECT 1
                  FROM search_profile_indexer_allow spa
                  WHERE spa.search_profile_id = profile_id
                    AND spa.indexer_instance_id = idx.indexer_instance_id
              )
          )
          AND (
              NOT profile_has_indexer_block
              OR NOT EXISTS (
                  SELECT 1
                  FROM search_profile_indexer_block spb
                  WHERE spb.search_profile_id = profile_id
                    AND spb.indexer_instance_id = idx.indexer_instance_id
              )
          )
          AND (
              NOT profile_has_tag_allow
              OR EXISTS (
                  SELECT 1
                  FROM indexer_instance_tag it
                  JOIN search_profile_tag_allow sta
                    ON sta.tag_id = it.tag_id
                  WHERE sta.search_profile_id = profile_id
                    AND it.indexer_instance_id = idx.indexer_instance_id
              )
          )
          AND (
              NOT profile_has_tag_block
              OR NOT EXISTS (
                  SELECT 1
                  FROM indexer_instance_tag it
                  JOIN search_profile_tag_block stb
                    ON stb.tag_id = it.tag_id
                  WHERE stb.search_profile_id = profile_id
                    AND it.indexer_instance_id = idx.indexer_instance_id
              )
          )
          AND (
              constraint_count = 0
              OR EXISTS (
                  SELECT 1
                  FROM indexer_instance_media_domain imd
                  WHERE imd.indexer_instance_id = idx.indexer_instance_id
                    AND imd.media_domain_id = ANY(allowed_domain_ids)
              )
          )
          AND (
              NOT has_policy_indexer_allow
              OR idx.indexer_instance_id = ANY(policy_allowed_indexer_ids)
          );
    END IF;

    IF runnable_indexer_ids IS NULL THEN
        runnable_indexer_ids := ARRAY[]::BIGINT[];
    END IF;
    runnable_indexer_count := COALESCE(array_length(runnable_indexer_ids, 1), 0);

    IF runnable_indexer_count = 0 THEN
        status_value := 'finished';
        finished_at_value := now();
    ELSE
        status_value := 'running';
        finished_at_value := NULL;
    END IF;

    new_search_request_public_id := gen_random_uuid();

    INSERT INTO search_request (
        search_request_public_id,
        user_id,
        search_profile_id,
        policy_set_id,
        policy_snapshot_id,
        requested_media_domain_id,
        effective_media_domain_id,
        query_text,
        query_type,
        torznab_mode,
        page_size,
        season_number,
        episode_number,
        status,
        finished_at
    )
    VALUES (
        new_search_request_public_id,
        actor_user_id,
        profile_id,
        request_policy_set_id,
        snapshot_id,
        requested_media_domain_id,
        effective_media_domain_id,
        resolved_query_text,
        resolved_query_type,
        resolved_torznab_mode,
        resolved_page_size,
        season_number_input,
        episode_number_input,
        status_value,
        finished_at_value
    )
    RETURNING search_request_id INTO search_request_id;

    IF auto_policy_set_created THEN
        UPDATE policy_set
        SET created_for_search_request_id = search_request_id
        WHERE policy_set_id = request_policy_set_id;
    END IF;

    UPDATE policy_snapshot
    SET ref_count = ref_count + 1
    WHERE policy_snapshot_id = snapshot_id;

    IF identifier_type_value IS NOT NULL THEN
        INSERT INTO search_request_identifier (
            search_request_id,
            id_type,
            id_value_normalized,
            id_value_raw
        )
        VALUES (
            search_request_id,
            identifier_type_value,
            identifier_normalized_value,
            identifier_raw_value
        );
    END IF;

    IF requested_cat_count > 0 THEN
        INSERT INTO search_request_torznab_category_requested (
            search_request_id,
            torznab_category_id
        )
        SELECT search_request_id, value
        FROM unnest(requested_cat_ids) AS value;
    END IF;

    IF effective_cat_count > 0 THEN
        INSERT INTO search_request_torznab_category_effective (
            search_request_id,
            torznab_category_id
        )
        SELECT search_request_id, value
        FROM unnest(effective_cat_ids) AS value;
    END IF;

    INSERT INTO search_page (
        search_request_id,
        page_number
    )
    VALUES (
        search_request_id,
        1
    );

    IF runnable_indexer_count > 0 THEN
        INSERT INTO search_request_indexer_run (
            search_request_id,
            indexer_instance_id,
            status,
            attempt_count,
            rate_limited_attempt_count,
            items_seen_count,
            items_emitted_count,
            canonical_added_count
        )
        SELECT
            search_request_id,
            value,
            'queued',
            0,
            0,
            0,
            0,
            0
        FROM unnest(runnable_indexer_ids) AS value;
    END IF;

    search_request_public_id := new_search_request_public_id;
    request_policy_set_public_id := request_policy_set_public_id_value;
    RETURN NEXT;
END;
$$;
