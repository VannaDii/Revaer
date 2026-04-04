-- Policy sets, rules, and snapshots.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'policy_scope') THEN
        CREATE TYPE policy_scope AS ENUM (
            'global',
            'user',
            'profile',
            'request'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'policy_rule_type') THEN
        CREATE TYPE policy_rule_type AS ENUM (
            'block_infohash_v1',
            'block_infohash_v2',
            'block_magnet',
            'block_title_regex',
            'block_release_group',
            'block_uploader',
            'block_tracker',
            'block_indexer_instance',
            'allow_release_group',
            'allow_title_regex',
            'allow_indexer_instance',
            'downrank_title_regex',
            'require_trust_tier_min',
            'require_media_domain',
            'prefer_indexer_instance',
            'prefer_trust_tier'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'policy_match_field') THEN
        CREATE TYPE policy_match_field AS ENUM (
            'infohash_v1',
            'infohash_v2',
            'magnet_hash',
            'title',
            'release_group',
            'uploader',
            'tracker',
            'indexer_instance_public_id',
            'media_domain_key',
            'trust_tier_key',
            'trust_tier_rank'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'policy_match_operator') THEN
        CREATE TYPE policy_match_operator AS ENUM (
            'eq',
            'contains',
            'regex',
            'starts_with',
            'ends_with',
            'in_set'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'policy_action') THEN
        CREATE TYPE policy_action AS ENUM (
            'drop_canonical',
            'drop_source',
            'downrank',
            'require',
            'prefer',
            'flag'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'policy_severity') THEN
        CREATE TYPE policy_severity AS ENUM ('hard', 'soft');
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS policy_set (
    policy_set_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    policy_set_public_id UUID NOT NULL,
    user_id BIGINT
        REFERENCES app_user (user_id),
    display_name VARCHAR(256) NOT NULL,
    scope policy_scope NOT NULL,
    is_enabled BOOLEAN NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 1000,
    is_auto_created BOOLEAN NOT NULL DEFAULT FALSE,
    created_for_search_request_id BIGINT,
    created_by_user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    updated_by_user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CONSTRAINT policy_set_public_id_uq UNIQUE (policy_set_public_id)
);

CREATE TABLE IF NOT EXISTS search_profile_policy_set (
    search_profile_policy_set_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_profile_id BIGINT NOT NULL
        REFERENCES search_profile (search_profile_id),
    policy_set_id BIGINT NOT NULL
        REFERENCES policy_set (policy_set_id),
    CONSTRAINT search_profile_policy_set_uq UNIQUE (
        search_profile_id,
        policy_set_id
    )
);

CREATE TABLE IF NOT EXISTS policy_rule (
    policy_rule_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    policy_set_id BIGINT NOT NULL
        REFERENCES policy_set (policy_set_id) ON DELETE CASCADE,
    policy_rule_public_id UUID NOT NULL,
    rule_type policy_rule_type NOT NULL,
    match_field policy_match_field NOT NULL,
    match_operator policy_match_operator NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 1000,
    match_value_text VARCHAR(512),
    match_value_int INTEGER,
    match_value_uuid UUID,
    value_set_id BIGINT,
    action policy_action NOT NULL,
    severity policy_severity NOT NULL,
    is_case_insensitive BOOLEAN NOT NULL DEFAULT TRUE,
    is_disabled BOOLEAN NOT NULL DEFAULT FALSE,
    rationale VARCHAR(1024),
    expires_at TIMESTAMPTZ,
    immutable_flag BOOLEAN NOT NULL DEFAULT FALSE,
    created_by_user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    updated_by_user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT policy_rule_public_id_uq UNIQUE (policy_rule_public_id)
);

CREATE TABLE IF NOT EXISTS policy_rule_value_set (
    value_set_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    policy_rule_id BIGINT NOT NULL
        REFERENCES policy_rule (policy_rule_id),
    value_set_type value_set_type NOT NULL,
    CONSTRAINT policy_rule_value_set_uq UNIQUE (policy_rule_id)
);

CREATE TABLE IF NOT EXISTS policy_rule_value_set_item (
    value_set_item_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    value_set_id BIGINT NOT NULL
        REFERENCES policy_rule_value_set (value_set_id),
    value_text VARCHAR(256),
    value_bigint BIGINT,
    value_int INTEGER,
    value_uuid UUID,
    CONSTRAINT policy_rule_value_set_item_single_chk CHECK (
        (
            (value_text IS NOT NULL)::INT
            + (value_bigint IS NOT NULL)::INT
            + (value_int IS NOT NULL)::INT
            + (value_uuid IS NOT NULL)::INT
        ) = 1
    ),
    CONSTRAINT policy_rule_value_set_item_text_lc CHECK (
        value_text IS NULL OR value_text = lower(value_text)
    )
);

CREATE TABLE IF NOT EXISTS policy_snapshot (
    policy_snapshot_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    snapshot_hash CHAR(64) NOT NULL,
    ref_count INTEGER NOT NULL DEFAULT 0,
    excluded_disabled_count INTEGER NOT NULL DEFAULT 0,
    excluded_expired_count INTEGER NOT NULL DEFAULT 0,
    CONSTRAINT policy_snapshot_hash_uq UNIQUE (snapshot_hash)
);

CREATE TABLE IF NOT EXISTS policy_snapshot_rule (
    policy_snapshot_rule_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    policy_snapshot_id BIGINT NOT NULL
        REFERENCES policy_snapshot (policy_snapshot_id) ON DELETE CASCADE,
    policy_rule_public_id UUID NOT NULL,
    rule_order INTEGER NOT NULL,
    CONSTRAINT policy_snapshot_rule_order_uq UNIQUE (
        policy_snapshot_id,
        rule_order
    ),
    CONSTRAINT policy_snapshot_rule_public_uq UNIQUE (
        policy_snapshot_id,
        policy_rule_public_id
    )
);

ALTER TABLE policy_rule
    ADD CONSTRAINT policy_rule_value_set_fk
        FOREIGN KEY (value_set_id)
        REFERENCES policy_rule_value_set (value_set_id);

CREATE UNIQUE INDEX IF NOT EXISTS policy_rule_value_set_item_text_uq
ON policy_rule_value_set_item (value_set_id, value_text)
WHERE value_text IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS policy_rule_value_set_item_bigint_uq
ON policy_rule_value_set_item (value_set_id, value_bigint)
WHERE value_bigint IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS policy_rule_value_set_item_int_uq
ON policy_rule_value_set_item (value_set_id, value_int)
WHERE value_int IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS policy_rule_value_set_item_uuid_uq
ON policy_rule_value_set_item (value_set_id, value_uuid)
WHERE value_uuid IS NOT NULL;
