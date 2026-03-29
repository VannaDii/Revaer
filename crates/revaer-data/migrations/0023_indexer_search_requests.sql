-- Search request tables and enums.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'query_type') THEN
        CREATE TYPE query_type AS ENUM (
            'free_text',
            'imdb',
            'tmdb',
            'tvdb',
            'season_episode'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'torznab_mode') THEN
        CREATE TYPE torznab_mode AS ENUM ('generic', 'tv', 'movie');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'search_status') THEN
        CREATE TYPE search_status AS ENUM (
            'running',
            'canceled',
            'finished',
            'failed'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'failure_class') THEN
        CREATE TYPE failure_class AS ENUM (
            'coordinator_error',
            'db_error',
            'auth_error',
            'invalid_request',
            'timeout',
            'canceled_by_system'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'run_status') THEN
        CREATE TYPE run_status AS ENUM (
            'queued',
            'running',
            'finished',
            'failed',
            'canceled'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'cursor_type') THEN
        CREATE TYPE cursor_type AS ENUM (
            'offset_limit',
            'page_number',
            'since_time',
            'opaque_token'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'observation_attr_key') THEN
        CREATE TYPE observation_attr_key AS ENUM (
            'tracker_name',
            'tracker_category',
            'tracker_subcategory',
            'size_bytes_reported',
            'files_count',
            'imdb_id',
            'tmdb_id',
            'tvdb_id',
            'season',
            'episode',
            'year',
            'release_group',
            'freeleech',
            'internal_flag',
            'scene_flag',
            'minimum_ratio',
            'minimum_seed_time_hours',
            'language_primary',
            'subtitles_primary'
        );
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS search_request (
    search_request_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_request_public_id UUID NOT NULL,
    user_id BIGINT
        REFERENCES app_user (user_id),
    search_profile_id BIGINT
        REFERENCES search_profile (search_profile_id),
    policy_set_id BIGINT
        REFERENCES policy_set (policy_set_id),
    policy_snapshot_id BIGINT NOT NULL
        REFERENCES policy_snapshot (policy_snapshot_id),
    requested_media_domain_id BIGINT
        REFERENCES media_domain (media_domain_id),
    effective_media_domain_id BIGINT
        REFERENCES media_domain (media_domain_id),
    query_text VARCHAR(512) NOT NULL,
    query_type query_type NOT NULL,
    torznab_mode torznab_mode,
    page_size INTEGER NOT NULL DEFAULT 50
        CHECK (page_size BETWEEN 10 AND 200),
    season_number INTEGER,
    episode_number INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    canceled_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    status search_status NOT NULL,
    failure_class failure_class,
    error_detail VARCHAR(1024),
    CONSTRAINT search_request_public_id_uq UNIQUE (search_request_public_id),
    CONSTRAINT search_request_season_number_chk CHECK (
        season_number IS NULL OR season_number >= 0
    ),
    CONSTRAINT search_request_episode_number_chk CHECK (
        episode_number IS NULL OR episode_number >= 0
    ),
    CONSTRAINT search_request_season_episode_mode_chk CHECK (
        (
            torznab_mode IS NULL
            AND (
                (query_type = 'season_episode'
                    AND season_number IS NOT NULL
                    AND episode_number IS NOT NULL)
                OR (query_type <> 'season_episode'
                    AND season_number IS NULL
                    AND episode_number IS NULL)
            )
        )
        OR (
            torznab_mode = 'tv'
            AND (episode_number IS NULL OR season_number IS NOT NULL)
        )
        OR (
            torznab_mode IN ('generic', 'movie')
            AND season_number IS NULL
            AND episode_number IS NULL
        )
    ),
    CONSTRAINT search_request_finished_at_chk CHECK (
        (status IN ('finished', 'failed', 'canceled') AND finished_at IS NOT NULL)
        OR (status = 'running' AND finished_at IS NULL)
    ),
    CONSTRAINT search_request_canceled_at_chk CHECK (
        (status = 'canceled' AND canceled_at IS NOT NULL)
        OR (status <> 'canceled' AND canceled_at IS NULL)
    ),
    CONSTRAINT search_request_failure_class_chk CHECK (
        (status = 'failed' AND failure_class IS NOT NULL)
        OR (status <> 'failed' AND failure_class IS NULL)
    )
);

CREATE TABLE IF NOT EXISTS search_request_identifier (
    search_request_identifier_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_request_id BIGINT NOT NULL
        REFERENCES search_request (search_request_id) ON DELETE CASCADE,
    id_type identifier_type NOT NULL,
    id_value_normalized VARCHAR(32) NOT NULL,
    id_value_raw VARCHAR(64) NOT NULL,
    CONSTRAINT search_request_identifier_uq UNIQUE (
        search_request_id,
        id_type
    ),
    CONSTRAINT search_request_identifier_imdb_chk CHECK (
        id_type <> 'imdb'
        OR id_value_normalized ~ '^tt[0-9]{7,9}$'
    ),
    CONSTRAINT search_request_identifier_tmdb_chk CHECK (
        id_type <> 'tmdb'
        OR id_value_normalized ~ '^[0-9]{1,10}$'
    ),
    CONSTRAINT search_request_identifier_tvdb_chk CHECK (
        id_type <> 'tvdb'
        OR id_value_normalized ~ '^[0-9]{1,10}$'
    )
);

CREATE TABLE IF NOT EXISTS search_request_torznab_category_requested (
    search_request_torznab_category_requested_id BIGINT
        GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_request_id BIGINT NOT NULL
        REFERENCES search_request (search_request_id) ON DELETE CASCADE,
    torznab_category_id BIGINT NOT NULL
        REFERENCES torznab_category (torznab_category_id),
    CONSTRAINT search_request_torznab_category_requested_uq UNIQUE (
        search_request_id,
        torznab_category_id
    )
);

CREATE TABLE IF NOT EXISTS search_request_torznab_category_effective (
    search_request_torznab_category_effective_id BIGINT
        GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_request_id BIGINT NOT NULL
        REFERENCES search_request (search_request_id) ON DELETE CASCADE,
    torznab_category_id BIGINT NOT NULL
        REFERENCES torznab_category (torznab_category_id),
    CONSTRAINT search_request_torznab_category_effective_uq UNIQUE (
        search_request_id,
        torznab_category_id
    )
);

CREATE TABLE IF NOT EXISTS search_request_indexer_run (
    search_request_indexer_run_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_request_id BIGINT NOT NULL
        REFERENCES search_request (search_request_id) ON DELETE CASCADE,
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    next_attempt_at TIMESTAMPTZ,
    attempt_count INTEGER NOT NULL DEFAULT 0
        CHECK (attempt_count >= 0),
    rate_limited_attempt_count INTEGER NOT NULL DEFAULT 0
        CHECK (rate_limited_attempt_count >= 0),
    last_error_class error_class,
    last_rate_limit_scope rate_limit_scope,
    last_correlation_id UUID,
    status run_status NOT NULL,
    error_class error_class,
    error_detail VARCHAR(1024),
    items_seen_count INTEGER NOT NULL DEFAULT 0
        CHECK (items_seen_count >= 0),
    items_emitted_count INTEGER NOT NULL DEFAULT 0
        CHECK (items_emitted_count >= 0),
    canonical_added_count INTEGER NOT NULL DEFAULT 0
        CHECK (canonical_added_count >= 0),
    CONSTRAINT search_request_indexer_run_uq UNIQUE (
        search_request_id,
        indexer_instance_id
    ),
    CONSTRAINT search_request_indexer_run_started_at_chk CHECK (
        (status = 'queued' AND started_at IS NULL)
        OR (status <> 'queued' AND started_at IS NOT NULL)
    ),
    CONSTRAINT search_request_indexer_run_finished_at_chk CHECK (
        (status IN ('finished', 'failed', 'canceled') AND finished_at IS NOT NULL)
        OR (status IN ('queued', 'running') AND finished_at IS NULL)
    ),
    CONSTRAINT search_request_indexer_run_error_class_chk CHECK (
        (status = 'failed' AND error_class IS NOT NULL)
        OR (status <> 'failed' AND error_class IS NULL)
    ),
    CONSTRAINT search_request_indexer_run_rate_limit_scope_chk CHECK (
        (last_error_class = 'rate_limited' AND last_rate_limit_scope IS NOT NULL)
        OR (last_error_class IS DISTINCT FROM 'rate_limited' AND last_rate_limit_scope IS NULL)
    )
);

CREATE TABLE IF NOT EXISTS search_request_indexer_run_correlation (
    search_request_indexer_run_correlation_id BIGINT
        GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_request_indexer_run_id BIGINT NOT NULL
        REFERENCES search_request_indexer_run (search_request_indexer_run_id)
        ON DELETE CASCADE,
    correlation_id UUID NOT NULL,
    page_number INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT search_request_indexer_run_correlation_uq UNIQUE (
        search_request_indexer_run_id,
        correlation_id
    ),
    CONSTRAINT search_request_indexer_run_correlation_page_chk CHECK (
        page_number IS NULL OR page_number >= 1
    )
);

CREATE TABLE IF NOT EXISTS indexer_run_cursor (
    indexer_run_cursor_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_request_indexer_run_id BIGINT NOT NULL
        REFERENCES search_request_indexer_run (search_request_indexer_run_id)
        ON DELETE CASCADE,
    cursor_type cursor_type NOT NULL,
    "offset" INTEGER,
    "limit" INTEGER,
    page INTEGER,
    since TIMESTAMPTZ,
    opaque_token VARCHAR(1024),
    CONSTRAINT indexer_run_cursor_uq UNIQUE (search_request_indexer_run_id),
    CONSTRAINT indexer_run_cursor_offset_chk CHECK (
        "offset" IS NULL OR "offset" >= 0
    ),
    CONSTRAINT indexer_run_cursor_limit_chk CHECK (
        "limit" IS NULL OR "limit" > 0
    ),
    CONSTRAINT indexer_run_cursor_page_chk CHECK (
        page IS NULL OR page > 0
    ),
    CONSTRAINT indexer_run_cursor_type_fields_chk CHECK (
        (
            cursor_type = 'offset_limit'
            AND "offset" IS NOT NULL
            AND "limit" IS NOT NULL
            AND page IS NULL
            AND since IS NULL
            AND opaque_token IS NULL
        )
        OR (
            cursor_type = 'page_number'
            AND page IS NOT NULL
            AND "offset" IS NULL
            AND "limit" IS NULL
            AND since IS NULL
            AND opaque_token IS NULL
        )
        OR (
            cursor_type = 'since_time'
            AND since IS NOT NULL
            AND "offset" IS NULL
            AND "limit" IS NULL
            AND page IS NULL
            AND opaque_token IS NULL
        )
        OR (
            cursor_type = 'opaque_token'
            AND opaque_token IS NOT NULL
            AND "offset" IS NULL
            AND "limit" IS NULL
            AND page IS NULL
            AND since IS NULL
        )
    )
);

CREATE TABLE IF NOT EXISTS search_request_canonical (
    search_request_canonical_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_request_id BIGINT NOT NULL
        REFERENCES search_request (search_request_id) ON DELETE CASCADE,
    canonical_torrent_id BIGINT NOT NULL
        REFERENCES canonical_torrent (canonical_torrent_id),
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT search_request_canonical_uq UNIQUE (
        search_request_id,
        canonical_torrent_id
    )
);

CREATE TABLE IF NOT EXISTS search_page (
    search_page_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_request_id BIGINT NOT NULL
        REFERENCES search_request (search_request_id) ON DELETE CASCADE,
    page_number INTEGER NOT NULL
        CHECK (page_number >= 1),
    sealed_at TIMESTAMPTZ,
    CONSTRAINT search_page_uq UNIQUE (search_request_id, page_number)
);

CREATE TABLE IF NOT EXISTS search_page_item (
    search_page_item_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_page_id BIGINT NOT NULL
        REFERENCES search_page (search_page_id) ON DELETE CASCADE,
    search_request_canonical_id BIGINT NOT NULL
        REFERENCES search_request_canonical (search_request_canonical_id)
        ON DELETE CASCADE,
    position INTEGER NOT NULL
        CHECK (position >= 1),
    CONSTRAINT search_page_item_position_uq UNIQUE (search_page_id, position),
    CONSTRAINT search_page_item_canonical_uq UNIQUE (search_request_canonical_id)
);

CREATE TABLE IF NOT EXISTS search_request_source_observation (
    observation_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_request_id BIGINT NOT NULL
        REFERENCES search_request (search_request_id) ON DELETE CASCADE,
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id),
    canonical_torrent_id BIGINT
        REFERENCES canonical_torrent (canonical_torrent_id),
    canonical_torrent_source_id BIGINT
        REFERENCES canonical_torrent_source (canonical_torrent_source_id),
    observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    seeders INTEGER,
    leechers INTEGER,
    published_at TIMESTAMPTZ,
    uploader VARCHAR(256),
    source_guid VARCHAR(256),
    details_url VARCHAR(2048),
    download_url VARCHAR(2048),
    magnet_uri VARCHAR(2048),
    title_raw VARCHAR(512) NOT NULL,
    size_bytes BIGINT,
    infohash_v1 CHAR(40),
    infohash_v2 CHAR(64),
    magnet_hash CHAR(64),
    guid_conflict BOOLEAN NOT NULL DEFAULT FALSE,
    was_downranked BOOLEAN NOT NULL DEFAULT FALSE,
    was_flagged BOOLEAN NOT NULL DEFAULT FALSE,
    CONSTRAINT search_request_source_observation_seeders_chk CHECK (
        seeders IS NULL OR seeders >= 0
    ),
    CONSTRAINT search_request_source_observation_leechers_chk CHECK (
        leechers IS NULL OR leechers >= 0
    ),
    CONSTRAINT search_request_source_observation_size_bytes_chk CHECK (
        size_bytes IS NULL OR size_bytes >= 0
    ),
    CONSTRAINT search_request_source_observation_infohash_v1_chk CHECK (
        infohash_v1 IS NULL OR infohash_v1 ~ '^[0-9a-f]{40}$'
    ),
    CONSTRAINT search_request_source_observation_infohash_v2_chk CHECK (
        infohash_v2 IS NULL OR infohash_v2 ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT search_request_source_observation_magnet_hash_chk CHECK (
        magnet_hash IS NULL OR magnet_hash ~ '^[0-9a-f]{64}$'
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS search_request_source_observation_guid_uq
    ON search_request_source_observation (
        search_request_id,
        indexer_instance_id,
        source_guid
    )
    WHERE source_guid IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS search_request_source_observation_source_uq
    ON search_request_source_observation (
        search_request_id,
        indexer_instance_id,
        canonical_torrent_source_id
    )
    WHERE source_guid IS NULL;

CREATE TABLE IF NOT EXISTS search_request_source_observation_attr (
    observation_attr_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    observation_id BIGINT NOT NULL
        REFERENCES search_request_source_observation (observation_id)
        ON DELETE CASCADE,
    attr_key observation_attr_key NOT NULL,
    value_text VARCHAR(512),
    value_int INTEGER,
    value_bigint BIGINT,
    value_numeric NUMERIC(12, 4),
    value_bool BOOLEAN,
    value_uuid UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT search_request_source_observation_attr_uq UNIQUE (
        observation_id,
        attr_key
    ),
    CONSTRAINT search_request_source_observation_attr_single_value_chk CHECK (
        (
            (value_text IS NOT NULL)::INT
            + (value_int IS NOT NULL)::INT
            + (value_bigint IS NOT NULL)::INT
            + (value_numeric IS NOT NULL)::INT
            + (value_bool IS NOT NULL)::INT
            + (value_uuid IS NOT NULL)::INT
        ) = 1
    ),
    CONSTRAINT search_request_source_observation_attr_key_type_chk CHECK (
        (attr_key IN ('tracker_name', 'release_group', 'language_primary', 'subtitles_primary', 'imdb_id')
            AND value_text IS NOT NULL)
        OR (attr_key = 'size_bytes_reported' AND value_bigint IS NOT NULL)
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
            ) AND value_int IS NOT NULL
        )
        OR (attr_key = 'minimum_ratio' AND value_numeric IS NOT NULL)
        OR (attr_key IN ('freeleech', 'internal_flag', 'scene_flag') AND value_bool IS NOT NULL)
    ),
    CONSTRAINT search_request_source_observation_attr_imdb_chk CHECK (
        attr_key <> 'imdb_id'
        OR value_text ~ '^tt[0-9]{7,9}$'
    ),
    CONSTRAINT search_request_source_observation_attr_tmdb_chk CHECK (
        attr_key <> 'tmdb_id'
        OR (value_int IS NOT NULL AND value_int > 0)
    ),
    CONSTRAINT search_request_source_observation_attr_tvdb_chk CHECK (
        attr_key <> 'tvdb_id'
        OR (value_int IS NOT NULL AND value_int > 0)
    ),
    CONSTRAINT search_request_source_observation_attr_tracker_category_chk CHECK (
        attr_key <> 'tracker_category'
        OR (value_int IS NOT NULL AND value_int >= 0)
    ),
    CONSTRAINT search_request_source_observation_attr_tracker_subcategory_chk CHECK (
        attr_key <> 'tracker_subcategory'
        OR (value_int IS NOT NULL AND value_int >= 0)
    ),
    CONSTRAINT search_request_source_observation_attr_files_count_chk CHECK (
        attr_key <> 'files_count'
        OR (value_int IS NOT NULL AND value_int >= 0)
    ),
    CONSTRAINT search_request_source_observation_attr_season_chk CHECK (
        attr_key <> 'season'
        OR (value_int IS NOT NULL AND value_int >= 0)
    ),
    CONSTRAINT search_request_source_observation_attr_episode_chk CHECK (
        attr_key <> 'episode'
        OR (value_int IS NOT NULL AND value_int >= 0)
    ),
    CONSTRAINT search_request_source_observation_attr_year_chk CHECK (
        attr_key <> 'year'
        OR (value_int IS NOT NULL AND value_int >= 0)
    ),
    CONSTRAINT search_request_source_observation_attr_min_seed_time_chk CHECK (
        attr_key <> 'minimum_seed_time_hours'
        OR (value_int IS NOT NULL AND value_int >= 0)
    ),
    CONSTRAINT search_request_source_observation_attr_size_bytes_chk CHECK (
        attr_key <> 'size_bytes_reported'
        OR (value_bigint IS NOT NULL AND value_bigint >= 0)
    ),
    CONSTRAINT search_request_source_observation_attr_min_ratio_chk CHECK (
        attr_key <> 'minimum_ratio'
        OR (value_numeric IS NOT NULL AND value_numeric >= 0)
    )
);
