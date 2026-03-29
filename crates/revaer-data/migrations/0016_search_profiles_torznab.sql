-- Search profiles and Torznab instances.

CREATE TABLE IF NOT EXISTS search_profile (
    search_profile_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_profile_public_id UUID NOT NULL,
    user_id BIGINT
        REFERENCES app_user (user_id),
    display_name VARCHAR(256) NOT NULL,
    is_default BOOLEAN NOT NULL,
    page_size INTEGER NOT NULL DEFAULT 50
        CHECK (page_size BETWEEN 10 AND 200),
    default_media_domain_id BIGINT
        REFERENCES media_domain (media_domain_id),
    created_by_user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    updated_by_user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CONSTRAINT search_profile_public_id_uq UNIQUE (search_profile_public_id)
);

CREATE TABLE IF NOT EXISTS search_profile_media_domain (
    search_profile_media_domain_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_profile_id BIGINT NOT NULL
        REFERENCES search_profile (search_profile_id),
    media_domain_id BIGINT NOT NULL
        REFERENCES media_domain (media_domain_id),
    CONSTRAINT search_profile_media_domain_uq UNIQUE (
        search_profile_id,
        media_domain_id
    )
);

CREATE TABLE IF NOT EXISTS search_profile_trust_tier (
    search_profile_trust_tier_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_profile_id BIGINT NOT NULL
        REFERENCES search_profile (search_profile_id),
    trust_tier_id BIGINT NOT NULL
        REFERENCES trust_tier (trust_tier_id),
    weight_override INTEGER
        CHECK (weight_override IS NULL OR weight_override BETWEEN -50 AND 50),
    CONSTRAINT search_profile_trust_tier_uq UNIQUE (
        search_profile_id,
        trust_tier_id
    )
);

CREATE TABLE IF NOT EXISTS search_profile_indexer_allow (
    search_profile_indexer_allow_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_profile_id BIGINT NOT NULL
        REFERENCES search_profile (search_profile_id),
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id),
    CONSTRAINT search_profile_indexer_allow_uq UNIQUE (
        search_profile_id,
        indexer_instance_id
    )
);

CREATE TABLE IF NOT EXISTS search_profile_indexer_block (
    search_profile_indexer_block_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_profile_id BIGINT NOT NULL
        REFERENCES search_profile (search_profile_id),
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id),
    CONSTRAINT search_profile_indexer_block_uq UNIQUE (
        search_profile_id,
        indexer_instance_id
    )
);

CREATE TABLE IF NOT EXISTS search_profile_tag_allow (
    search_profile_tag_allow_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_profile_id BIGINT NOT NULL
        REFERENCES search_profile (search_profile_id),
    tag_id BIGINT NOT NULL
        REFERENCES tag (tag_id),
    CONSTRAINT search_profile_tag_allow_uq UNIQUE (search_profile_id, tag_id)
);

CREATE TABLE IF NOT EXISTS search_profile_tag_block (
    search_profile_tag_block_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_profile_id BIGINT NOT NULL
        REFERENCES search_profile (search_profile_id),
    tag_id BIGINT NOT NULL
        REFERENCES tag (tag_id),
    CONSTRAINT search_profile_tag_block_uq UNIQUE (search_profile_id, tag_id)
);

CREATE TABLE IF NOT EXISTS search_profile_tag_prefer (
    search_profile_tag_prefer_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_profile_id BIGINT NOT NULL
        REFERENCES search_profile (search_profile_id),
    tag_id BIGINT NOT NULL
        REFERENCES tag (tag_id),
    weight_override INTEGER DEFAULT 5
        CHECK (weight_override IS NULL OR weight_override BETWEEN -50 AND 50),
    CONSTRAINT search_profile_tag_prefer_uq UNIQUE (search_profile_id, tag_id)
);

CREATE TABLE IF NOT EXISTS torznab_instance (
    torznab_instance_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_profile_id BIGINT NOT NULL
        REFERENCES search_profile (search_profile_id),
    torznab_instance_public_id UUID NOT NULL,
    display_name VARCHAR(256) NOT NULL,
    api_key_hash TEXT NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CONSTRAINT torznab_instance_public_id_uq UNIQUE (torznab_instance_public_id),
    CONSTRAINT torznab_instance_display_name_uq UNIQUE (display_name)
);
