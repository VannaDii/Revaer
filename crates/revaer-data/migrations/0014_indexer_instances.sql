-- Indexer instances, routing policies, and RSS.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'indexer_instance_migration_state') THEN
        CREATE TYPE indexer_instance_migration_state AS ENUM (
            'ready',
            'needs_secret',
            'test_failed',
            'unmapped_definition',
            'duplicate_suspected'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'import_source_system') THEN
        CREATE TYPE import_source_system AS ENUM ('prowlarr');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'import_payload_format') THEN
        CREATE TYPE import_payload_format AS ENUM ('prowlarr_indexer_json_v1');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'error_class') THEN
        CREATE TYPE error_class AS ENUM (
            'dns',
            'tls',
            'timeout',
            'connection_refused',
            'http_403',
            'http_429',
            'http_5xx',
            'parse_error',
            'auth_error',
            'cf_challenge',
            'rate_limited',
            'unknown'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'routing_policy_mode') THEN
        CREATE TYPE routing_policy_mode AS ENUM (
            'direct',
            'http_proxy',
            'socks_proxy',
            'flaresolverr',
            'vpn_route',
            'tor'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'routing_param_key') THEN
        CREATE TYPE routing_param_key AS ENUM (
            'verify_tls',
            'proxy_host',
            'proxy_port',
            'proxy_username',
            'proxy_use_tls',
            'http_proxy_auth',
            'socks_host',
            'socks_port',
            'socks_username',
            'socks_proxy_auth',
            'fs_url',
            'fs_timeout_ms',
            'fs_session_ttl_seconds',
            'fs_user_agent'
        );
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS routing_policy (
    routing_policy_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    routing_policy_public_id UUID NOT NULL,
    display_name VARCHAR(256) NOT NULL,
    mode routing_policy_mode NOT NULL,
    created_by_user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    updated_by_user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CONSTRAINT routing_policy_public_id_uq UNIQUE (routing_policy_public_id),
    CONSTRAINT routing_policy_display_name_uq UNIQUE (display_name)
);

CREATE TABLE IF NOT EXISTS routing_policy_parameter (
    routing_policy_parameter_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    routing_policy_id BIGINT NOT NULL
        REFERENCES routing_policy (routing_policy_id),
    param_key routing_param_key NOT NULL,
    value_plain VARCHAR(2048),
    value_int INTEGER,
    value_bool BOOLEAN,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT routing_policy_parameter_uq UNIQUE (routing_policy_id, param_key)
);

CREATE TABLE IF NOT EXISTS indexer_instance (
    indexer_instance_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_instance_public_id UUID NOT NULL,
    indexer_definition_id BIGINT NOT NULL
        REFERENCES indexer_definition (indexer_definition_id),
    display_name VARCHAR(256) NOT NULL,
    is_enabled BOOLEAN NOT NULL,
    migration_state indexer_instance_migration_state,
    migration_detail VARCHAR(256),
    enable_rss BOOLEAN NOT NULL DEFAULT TRUE,
    enable_automatic_search BOOLEAN NOT NULL DEFAULT TRUE,
    enable_interactive_search BOOLEAN NOT NULL DEFAULT TRUE,
    priority INTEGER NOT NULL DEFAULT 50
        CHECK (priority BETWEEN 0 AND 100),
    trust_tier_key trust_tier_key,
    routing_policy_id BIGINT
        REFERENCES routing_policy (routing_policy_id),
    connect_timeout_ms INTEGER NOT NULL DEFAULT 5000
        CHECK (connect_timeout_ms BETWEEN 500 AND 60000),
    read_timeout_ms INTEGER NOT NULL DEFAULT 15000
        CHECK (read_timeout_ms BETWEEN 500 AND 120000),
    max_parallel_requests INTEGER NOT NULL DEFAULT 2
        CHECK (max_parallel_requests BETWEEN 1 AND 16),
    created_by_user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    updated_by_user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CONSTRAINT indexer_instance_public_id_uq UNIQUE (indexer_instance_public_id),
    CONSTRAINT indexer_instance_display_name_uq UNIQUE (display_name)
);

CREATE TABLE IF NOT EXISTS indexer_instance_media_domain (
    indexer_instance_media_domain_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id),
    media_domain_id BIGINT NOT NULL
        REFERENCES media_domain (media_domain_id),
    CONSTRAINT indexer_instance_media_domain_uq UNIQUE (
        indexer_instance_id,
        media_domain_id
    )
);

CREATE TABLE IF NOT EXISTS indexer_instance_tag (
    indexer_instance_tag_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id),
    tag_id BIGINT NOT NULL
        REFERENCES tag (tag_id),
    CONSTRAINT indexer_instance_tag_uq UNIQUE (indexer_instance_id, tag_id)
);

CREATE TABLE IF NOT EXISTS indexer_rss_subscription (
    indexer_rss_subscription_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id),
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    interval_seconds INTEGER NOT NULL DEFAULT 900
        CHECK (interval_seconds BETWEEN 300 AND 86400),
    last_polled_at TIMESTAMPTZ,
    next_poll_at TIMESTAMPTZ,
    backoff_seconds INTEGER,
    last_error_class error_class,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT indexer_rss_subscription_instance_uq UNIQUE (indexer_instance_id),
    CONSTRAINT indexer_rss_subscription_next_poll_chk CHECK (
        (is_enabled = TRUE AND next_poll_at IS NOT NULL)
        OR (is_enabled = FALSE AND next_poll_at IS NULL)
    )
);

CREATE TABLE IF NOT EXISTS indexer_rss_item_seen (
    rss_item_seen_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id),
    item_guid VARCHAR(256),
    infohash_v1 CHAR(40),
    infohash_v2 CHAR(64),
    magnet_hash CHAR(64),
    first_seen_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT indexer_rss_item_seen_identifier_chk CHECK (
        item_guid IS NOT NULL
        OR infohash_v1 IS NOT NULL
        OR infohash_v2 IS NOT NULL
        OR magnet_hash IS NOT NULL
    ),
    CONSTRAINT indexer_rss_item_seen_infohash_v1_chk CHECK (
        infohash_v1 IS NULL OR infohash_v1 ~ '^[0-9a-f]{40}$'
    ),
    CONSTRAINT indexer_rss_item_seen_infohash_v2_chk CHECK (
        infohash_v2 IS NULL OR infohash_v2 ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT indexer_rss_item_seen_magnet_hash_chk CHECK (
        magnet_hash IS NULL OR magnet_hash ~ '^[0-9a-f]{64}$'
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS indexer_rss_item_seen_guid_uq
    ON indexer_rss_item_seen (indexer_instance_id, item_guid)
    WHERE item_guid IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS indexer_rss_item_seen_infohash_v2_uq
    ON indexer_rss_item_seen (indexer_instance_id, infohash_v2)
    WHERE infohash_v2 IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS indexer_rss_item_seen_infohash_v1_uq
    ON indexer_rss_item_seen (indexer_instance_id, infohash_v1)
    WHERE infohash_v1 IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS indexer_rss_item_seen_magnet_hash_uq
    ON indexer_rss_item_seen (indexer_instance_id, magnet_hash)
    WHERE magnet_hash IS NOT NULL;

CREATE TABLE IF NOT EXISTS indexer_instance_field_value (
    indexer_instance_field_value_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id),
    field_name VARCHAR(128) NOT NULL,
    field_type field_type NOT NULL,
    value_plain VARCHAR(2048),
    value_int INTEGER,
    value_decimal NUMERIC(12, 4),
    value_bool BOOLEAN,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by_user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    CONSTRAINT indexer_instance_field_value_uq UNIQUE (
        indexer_instance_id,
        field_name
    ),
    CONSTRAINT indexer_instance_field_name_lc CHECK (
        field_name = lower(field_name)
    ),
    CONSTRAINT indexer_instance_field_name_len_chk CHECK (
        length(field_name) BETWEEN 1 AND 128
    ),
    CONSTRAINT indexer_instance_field_value_secret_chk CHECK (
        field_type NOT IN ('password', 'api_key', 'cookie', 'token', 'header_value')
        OR (
            value_plain IS NULL
            AND value_int IS NULL
            AND value_decimal IS NULL
            AND value_bool IS NULL
        )
    ),
    CONSTRAINT indexer_instance_field_value_non_secret_chk CHECK (
        field_type IN ('password', 'api_key', 'cookie', 'token', 'header_value')
        OR (
            (
                (value_plain IS NOT NULL)::INT
                + (value_int IS NOT NULL)::INT
                + (value_decimal IS NOT NULL)::INT
                + (value_bool IS NOT NULL)::INT
            ) = 1
        )
    )
);

CREATE TABLE IF NOT EXISTS indexer_instance_import_blob (
    indexer_instance_import_blob_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id),
    source_system import_source_system NOT NULL,
    import_payload_text TEXT NOT NULL,
    import_payload_format import_payload_format NOT NULL,
    imported_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT indexer_instance_import_blob_uq UNIQUE (
        indexer_instance_id,
        source_system
    )
);
