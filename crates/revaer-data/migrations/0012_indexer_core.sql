-- Indexer ERD core tables (users, config, trust tiers, media domains, tags).

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'deployment_role') THEN
        CREATE TYPE deployment_role AS ENUM ('owner', 'admin', 'user');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'trust_tier_key') THEN
        CREATE TYPE trust_tier_key AS ENUM (
            'public',
            'semi_private',
            'private',
            'invite_only'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'media_domain_key') THEN
        CREATE TYPE media_domain_key AS ENUM (
            'movies',
            'tv',
            'audiobooks',
            'ebooks',
            'software',
            'adult_movies',
            'adult_scenes'
        );
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS app_user (
    user_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_public_id UUID NOT NULL,
    email VARCHAR(320) NOT NULL,
    email_normalized VARCHAR(320) NOT NULL,
    is_email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    display_name VARCHAR(256) NOT NULL,
    role deployment_role NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT app_user_user_public_id_uq UNIQUE (user_public_id),
    CONSTRAINT app_user_email_uq UNIQUE (email),
    CONSTRAINT app_user_email_normalized_uq UNIQUE (email_normalized),
    CONSTRAINT app_user_email_normalized_lc CHECK (
        email_normalized = lower(trim(email_normalized))
    )
);

CREATE TABLE IF NOT EXISTS deployment_config (
    deployment_config_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    default_page_size INTEGER NOT NULL DEFAULT 50
        CHECK (default_page_size BETWEEN 10 AND 200),
    retention_search_days INTEGER NOT NULL DEFAULT 7
        CHECK (retention_search_days BETWEEN 1 AND 90),
    retention_health_events_days INTEGER NOT NULL DEFAULT 14
        CHECK (retention_health_events_days BETWEEN 1 AND 90),
    retention_reputation_days INTEGER NOT NULL DEFAULT 180
        CHECK (retention_reputation_days BETWEEN 30 AND 3650),
    retention_outbound_request_log_days INTEGER NOT NULL DEFAULT 14
        CHECK (retention_outbound_request_log_days BETWEEN 1 AND 90),
    retention_source_metadata_conflict_days INTEGER NOT NULL DEFAULT 30
        CHECK (retention_source_metadata_conflict_days BETWEEN 1 AND 365),
    retention_source_metadata_conflict_audit_days INTEGER NOT NULL DEFAULT 90
        CHECK (retention_source_metadata_conflict_audit_days BETWEEN 7 AND 3650),
    retention_rss_item_seen_days INTEGER NOT NULL DEFAULT 30
        CHECK (retention_rss_item_seen_days BETWEEN 1 AND 365),
    connectivity_refresh_seconds INTEGER
        CHECK (connectivity_refresh_seconds BETWEEN 30 AND 3600),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS deployment_maintenance_state (
    deployment_maintenance_state_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    rss_subscription_backfill_completed_at TIMESTAMPTZ,
    last_updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS trust_tier (
    trust_tier_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    trust_tier_key trust_tier_key NOT NULL,
    display_name VARCHAR(256) NOT NULL,
    default_weight NUMERIC(12, 4) NOT NULL
        CHECK (default_weight BETWEEN -50 AND 50),
    rank SMALLINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT trust_tier_key_uq UNIQUE (trust_tier_key)
);

CREATE TABLE IF NOT EXISTS media_domain (
    media_domain_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    media_domain_key media_domain_key NOT NULL,
    display_name VARCHAR(256) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT media_domain_key_uq UNIQUE (media_domain_key)
);

CREATE TABLE IF NOT EXISTS tag (
    tag_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tag_public_id UUID NOT NULL,
    tag_key VARCHAR(128) NOT NULL,
    display_name VARCHAR(256) NOT NULL,
    created_by_user_id BIGINT NOT NULL,
    updated_by_user_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CONSTRAINT tag_key_uq UNIQUE (tag_key),
    CONSTRAINT tag_public_id_uq UNIQUE (tag_public_id),
    CONSTRAINT tag_key_lc CHECK (tag_key = lower(tag_key)),
    CONSTRAINT tag_created_by_fk FOREIGN KEY (created_by_user_id)
        REFERENCES app_user (user_id),
    CONSTRAINT tag_updated_by_fk FOREIGN KEY (updated_by_user_id)
        REFERENCES app_user (user_id)
);
