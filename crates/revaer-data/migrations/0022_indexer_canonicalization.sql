-- Canonical torrent schema and related enums.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'identifier_type') THEN
        CREATE TYPE identifier_type AS ENUM ('imdb', 'tmdb', 'tvdb');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'identity_strategy') THEN
        CREATE TYPE identity_strategy AS ENUM (
            'infohash_v1',
            'infohash_v2',
            'magnet_hash',
            'title_size_fallback'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'durable_source_attr_key') THEN
        CREATE TYPE durable_source_attr_key AS ENUM (
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
            'year'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'signal_key') THEN
        CREATE TYPE signal_key AS ENUM (
            'release_group',
            'resolution',
            'source_type',
            'codec',
            'audio_codec',
            'container',
            'language',
            'subtitles',
            'edition',
            'year',
            'season',
            'episode'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'disambiguation_rule_type') THEN
        CREATE TYPE disambiguation_rule_type AS ENUM ('prevent_merge');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'disambiguation_identity_type') THEN
        CREATE TYPE disambiguation_identity_type AS ENUM (
            'infohash_v1',
            'infohash_v2',
            'magnet_hash',
            'canonical_public_id'
        );
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS canonical_torrent (
    canonical_torrent_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    canonical_torrent_public_id UUID NOT NULL,
    identity_confidence NUMERIC(4, 3) NOT NULL
        CHECK (identity_confidence BETWEEN 0 AND 1),
    identity_strategy identity_strategy NOT NULL,
    infohash_v1 CHAR(40),
    infohash_v2 CHAR(64),
    magnet_hash CHAR(64),
    title_size_hash CHAR(64),
    imdb_id VARCHAR(16),
    tmdb_id INTEGER,
    tvdb_id INTEGER,
    ids_confidence NUMERIC(4, 3),
    title_display VARCHAR(512) NOT NULL,
    title_normalized VARCHAR(512) NOT NULL,
    size_bytes BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT canonical_torrent_public_id_uq UNIQUE (canonical_torrent_public_id),
    CONSTRAINT canonical_torrent_infohash_v1_chk CHECK (
        infohash_v1 IS NULL OR infohash_v1 ~ '^[0-9a-f]{40}$'
    ),
    CONSTRAINT canonical_torrent_infohash_v2_chk CHECK (
        infohash_v2 IS NULL OR infohash_v2 ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT canonical_torrent_magnet_hash_chk CHECK (
        magnet_hash IS NULL OR magnet_hash ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT canonical_torrent_title_size_hash_chk CHECK (
        title_size_hash IS NULL OR title_size_hash ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT canonical_torrent_imdb_id_chk CHECK (
        imdb_id IS NULL OR imdb_id ~ '^tt[0-9]{7,9}$'
    ),
    CONSTRAINT canonical_torrent_tmdb_id_chk CHECK (
        tmdb_id IS NULL OR tmdb_id > 0
    ),
    CONSTRAINT canonical_torrent_tvdb_id_chk CHECK (
        tvdb_id IS NULL OR tvdb_id > 0
    ),
    CONSTRAINT canonical_torrent_ids_confidence_chk CHECK (
        ids_confidence IS NULL OR ids_confidence BETWEEN 0 AND 1
    ),
    CONSTRAINT canonical_torrent_title_normalized_lc CHECK (
        title_normalized = lower(title_normalized)
    ),
    CONSTRAINT canonical_torrent_size_bytes_chk CHECK (
        size_bytes IS NULL OR size_bytes >= 0
    ),
    CONSTRAINT canonical_torrent_identity_strategy_chk CHECK (
        (identity_strategy = 'infohash_v2' AND infohash_v2 IS NOT NULL)
        OR (identity_strategy = 'infohash_v1' AND infohash_v1 IS NOT NULL)
        OR (identity_strategy = 'magnet_hash' AND magnet_hash IS NOT NULL)
        OR (identity_strategy = 'title_size_fallback' AND title_size_hash IS NOT NULL)
    ),
    CONSTRAINT canonical_torrent_title_size_requires_size_chk CHECK (
        title_size_hash IS NULL OR size_bytes IS NOT NULL
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS canonical_torrent_infohash_v2_uq
    ON canonical_torrent (infohash_v2)
    WHERE infohash_v2 IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS canonical_torrent_infohash_v1_uq
    ON canonical_torrent (infohash_v1)
    WHERE infohash_v1 IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS canonical_torrent_magnet_hash_uq
    ON canonical_torrent (magnet_hash)
    WHERE magnet_hash IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS canonical_torrent_title_size_hash_uq
    ON canonical_torrent (title_size_hash)
    WHERE title_size_hash IS NOT NULL;

CREATE TABLE IF NOT EXISTS canonical_size_rollup (
    canonical_size_rollup_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    canonical_torrent_id BIGINT NOT NULL
        REFERENCES canonical_torrent (canonical_torrent_id) ON DELETE CASCADE,
    sample_count INTEGER NOT NULL
        CHECK (sample_count > 0),
    size_median BIGINT NOT NULL
        CHECK (size_median > 0),
    size_min BIGINT NOT NULL
        CHECK (size_min > 0),
    size_max BIGINT NOT NULL
        CHECK (size_max > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT canonical_size_rollup_uq UNIQUE (canonical_torrent_id)
);

CREATE TABLE IF NOT EXISTS canonical_size_sample (
    canonical_size_sample_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    canonical_torrent_id BIGINT NOT NULL
        REFERENCES canonical_torrent (canonical_torrent_id) ON DELETE CASCADE,
    observed_at TIMESTAMPTZ NOT NULL,
    size_bytes BIGINT NOT NULL
        CHECK (size_bytes > 0),
    CONSTRAINT canonical_size_sample_uq UNIQUE (
        canonical_torrent_id,
        observed_at,
        size_bytes
    )
);

CREATE TABLE IF NOT EXISTS canonical_torrent_source (
    canonical_torrent_source_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id),
    canonical_torrent_source_public_id UUID NOT NULL,
    source_guid VARCHAR(256),
    infohash_v1 CHAR(40),
    infohash_v2 CHAR(64),
    magnet_hash CHAR(64),
    title_normalized VARCHAR(512) NOT NULL,
    size_bytes BIGINT,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_seeders INTEGER,
    last_seen_leechers INTEGER,
    last_seen_published_at TIMESTAMPTZ,
    last_seen_download_url VARCHAR(2048),
    last_seen_magnet_uri VARCHAR(2048),
    last_seen_details_url VARCHAR(2048),
    last_seen_uploader VARCHAR(256),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT canonical_torrent_source_public_id_uq UNIQUE (
        canonical_torrent_source_public_id
    ),
    CONSTRAINT canonical_torrent_source_infohash_v1_chk CHECK (
        infohash_v1 IS NULL OR infohash_v1 ~ '^[0-9a-f]{40}$'
    ),
    CONSTRAINT canonical_torrent_source_infohash_v2_chk CHECK (
        infohash_v2 IS NULL OR infohash_v2 ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT canonical_torrent_source_magnet_hash_chk CHECK (
        magnet_hash IS NULL OR magnet_hash ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT canonical_torrent_source_title_normalized_lc CHECK (
        title_normalized = lower(title_normalized)
    ),
    CONSTRAINT canonical_torrent_source_size_bytes_chk CHECK (
        size_bytes IS NULL OR size_bytes >= 0
    ),
    CONSTRAINT canonical_torrent_source_seen_seeders_chk CHECK (
        last_seen_seeders IS NULL OR last_seen_seeders >= 0
    ),
    CONSTRAINT canonical_torrent_source_seen_leechers_chk CHECK (
        last_seen_leechers IS NULL OR last_seen_leechers >= 0
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS canonical_torrent_source_guid_uq
    ON canonical_torrent_source (indexer_instance_id, source_guid)
    WHERE source_guid IS NOT NULL;

CREATE TABLE IF NOT EXISTS canonical_external_id (
    canonical_external_id_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    canonical_torrent_id BIGINT NOT NULL
        REFERENCES canonical_torrent (canonical_torrent_id) ON DELETE CASCADE,
    id_type identifier_type NOT NULL,
    id_value_text VARCHAR(16),
    id_value_int INTEGER,
    source_canonical_torrent_source_id BIGINT
        REFERENCES canonical_torrent_source (canonical_torrent_source_id),
    trust_tier_rank SMALLINT NOT NULL
        CHECK (trust_tier_rank >= 0),
    first_seen_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT canonical_external_id_single_value_chk CHECK (
        (
            (id_value_text IS NOT NULL)::INT
            + (id_value_int IS NOT NULL)::INT
        ) = 1
    ),
    CONSTRAINT canonical_external_id_imdb_chk CHECK (
        id_type <> 'imdb'
        OR (id_value_text IS NOT NULL AND id_value_text ~ '^tt[0-9]{7,9}$')
    ),
    CONSTRAINT canonical_external_id_tmdb_chk CHECK (
        id_type <> 'tmdb'
        OR (id_value_int IS NOT NULL AND id_value_int > 0)
    ),
    CONSTRAINT canonical_external_id_tvdb_chk CHECK (
        id_type <> 'tvdb'
        OR (id_value_int IS NOT NULL AND id_value_int > 0)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS canonical_external_id_text_uq
    ON canonical_external_id (canonical_torrent_id, id_type, id_value_text)
    WHERE id_value_text IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS canonical_external_id_int_uq
    ON canonical_external_id (canonical_torrent_id, id_type, id_value_int)
    WHERE id_value_int IS NOT NULL;

CREATE TABLE IF NOT EXISTS canonical_torrent_source_attr (
    canonical_torrent_source_attr_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    canonical_torrent_source_id BIGINT NOT NULL
        REFERENCES canonical_torrent_source (canonical_torrent_source_id)
        ON DELETE CASCADE,
    attr_key durable_source_attr_key NOT NULL,
    value_text VARCHAR(512),
    value_int INTEGER,
    value_bigint BIGINT,
    value_numeric NUMERIC(12, 4),
    value_bool BOOLEAN,
    CONSTRAINT canonical_torrent_source_attr_uq UNIQUE (
        canonical_torrent_source_id,
        attr_key
    ),
    CONSTRAINT canonical_torrent_source_attr_single_value_chk CHECK (
        (
            (value_text IS NOT NULL)::INT
            + (value_int IS NOT NULL)::INT
            + (value_bigint IS NOT NULL)::INT
            + (value_numeric IS NOT NULL)::INT
            + (value_bool IS NOT NULL)::INT
        ) = 1
    ),
    CONSTRAINT canonical_torrent_source_attr_key_type_chk CHECK (
        (attr_key IN ('tracker_name', 'imdb_id') AND value_text IS NOT NULL)
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
                'tvdb_id'
            ) AND value_int IS NOT NULL
        )
    ),
    CONSTRAINT canonical_torrent_source_attr_imdb_chk CHECK (
        attr_key <> 'imdb_id'
        OR value_text ~ '^tt[0-9]{7,9}$'
    ),
    CONSTRAINT canonical_torrent_source_attr_tmdb_chk CHECK (
        attr_key <> 'tmdb_id'
        OR (value_int IS NOT NULL AND value_int > 0)
    ),
    CONSTRAINT canonical_torrent_source_attr_tvdb_chk CHECK (
        attr_key <> 'tvdb_id'
        OR (value_int IS NOT NULL AND value_int > 0)
    ),
    CONSTRAINT canonical_torrent_source_attr_tracker_category_chk CHECK (
        attr_key <> 'tracker_category'
        OR (value_int IS NOT NULL AND value_int >= 0)
    ),
    CONSTRAINT canonical_torrent_source_attr_tracker_subcategory_chk CHECK (
        attr_key <> 'tracker_subcategory'
        OR (value_int IS NOT NULL AND value_int >= 0)
    ),
    CONSTRAINT canonical_torrent_source_attr_files_count_chk CHECK (
        attr_key <> 'files_count'
        OR (value_int IS NOT NULL AND value_int >= 0)
    ),
    CONSTRAINT canonical_torrent_source_attr_season_chk CHECK (
        attr_key <> 'season'
        OR (value_int IS NOT NULL AND value_int >= 0)
    ),
    CONSTRAINT canonical_torrent_source_attr_episode_chk CHECK (
        attr_key <> 'episode'
        OR (value_int IS NOT NULL AND value_int >= 0)
    ),
    CONSTRAINT canonical_torrent_source_attr_year_chk CHECK (
        attr_key <> 'year'
        OR (value_int IS NOT NULL AND value_int >= 0)
    ),
    CONSTRAINT canonical_torrent_source_attr_size_bytes_chk CHECK (
        attr_key <> 'size_bytes_reported'
        OR (value_bigint IS NOT NULL AND value_bigint >= 0)
    )
);

CREATE TABLE IF NOT EXISTS canonical_torrent_signal (
    canonical_torrent_signal_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    canonical_torrent_id BIGINT NOT NULL
        REFERENCES canonical_torrent (canonical_torrent_id) ON DELETE CASCADE,
    signal_key signal_key NOT NULL,
    value_text VARCHAR(128),
    value_int INTEGER,
    confidence NUMERIC(4, 3) NOT NULL
        CHECK (confidence BETWEEN 0 AND 1),
    parser_version SMALLINT NOT NULL DEFAULT 1,
    CONSTRAINT canonical_torrent_signal_uq UNIQUE (
        canonical_torrent_id,
        signal_key,
        value_text,
        value_int
    ),
    CONSTRAINT canonical_torrent_signal_single_value_chk CHECK (
        (
            (value_text IS NOT NULL)::INT
            + (value_int IS NOT NULL)::INT
        ) = 1
    )
);

CREATE TABLE IF NOT EXISTS canonical_disambiguation_rule (
    canonical_disambiguation_rule_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    created_by_user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    rule_type disambiguation_rule_type NOT NULL,
    identity_left_type disambiguation_identity_type NOT NULL,
    identity_left_value_text VARCHAR(64),
    identity_left_value_uuid UUID,
    identity_right_type disambiguation_identity_type NOT NULL,
    identity_right_value_text VARCHAR(64),
    identity_right_value_uuid UUID,
    reason VARCHAR(256),
    CONSTRAINT canonical_disambiguation_rule_uq UNIQUE (
        identity_left_type,
        identity_left_value_text,
        identity_left_value_uuid,
        identity_right_type,
        identity_right_value_text,
        identity_right_value_uuid
    ),
    CONSTRAINT canonical_disambiguation_rule_left_value_chk CHECK (
        (
            (identity_left_value_text IS NOT NULL)::INT
            + (identity_left_value_uuid IS NOT NULL)::INT
        ) = 1
    ),
    CONSTRAINT canonical_disambiguation_rule_right_value_chk CHECK (
        (
            (identity_right_value_text IS NOT NULL)::INT
            + (identity_right_value_uuid IS NOT NULL)::INT
        ) = 1
    ),
    CONSTRAINT canonical_disambiguation_rule_left_type_chk CHECK (
        (identity_left_type = 'canonical_public_id' AND identity_left_value_uuid IS NOT NULL)
        OR (identity_left_type <> 'canonical_public_id' AND identity_left_value_text IS NOT NULL)
    ),
    CONSTRAINT canonical_disambiguation_rule_right_type_chk CHECK (
        (identity_right_type = 'canonical_public_id' AND identity_right_value_uuid IS NOT NULL)
        OR (identity_right_type <> 'canonical_public_id' AND identity_right_value_text IS NOT NULL)
    ),
    CONSTRAINT canonical_disambiguation_rule_left_hash_chk CHECK (
        identity_left_type = 'canonical_public_id'
        OR (identity_left_type = 'infohash_v1' AND identity_left_value_text ~ '^[0-9a-f]{40}$')
        OR (identity_left_type = 'infohash_v2' AND identity_left_value_text ~ '^[0-9a-f]{64}$')
        OR (identity_left_type = 'magnet_hash' AND identity_left_value_text ~ '^[0-9a-f]{64}$')
    ),
    CONSTRAINT canonical_disambiguation_rule_right_hash_chk CHECK (
        identity_right_type = 'canonical_public_id'
        OR (identity_right_type = 'infohash_v1' AND identity_right_value_text ~ '^[0-9a-f]{40}$')
        OR (identity_right_type = 'infohash_v2' AND identity_right_value_text ~ '^[0-9a-f]{64}$')
        OR (identity_right_type = 'magnet_hash' AND identity_right_value_text ~ '^[0-9a-f]{64}$')
    ),
    CONSTRAINT canonical_disambiguation_rule_distinct_chk CHECK (
        ROW(
            identity_left_type,
            identity_left_value_text,
            identity_left_value_uuid
        ) IS DISTINCT FROM ROW(
            identity_right_type,
            identity_right_value_text,
            identity_right_value_uuid
        )
    )
);
