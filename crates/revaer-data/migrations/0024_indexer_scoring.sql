-- Canonical scoring and best-source materializations.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'context_key_type') THEN
        CREATE TYPE context_key_type AS ENUM (
            'policy_snapshot',
            'search_profile',
            'search_request'
        );
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS canonical_torrent_source_base_score (
    canonical_torrent_source_base_score_id BIGINT
        GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    canonical_torrent_id BIGINT NOT NULL
        REFERENCES canonical_torrent (canonical_torrent_id) ON DELETE CASCADE,
    canonical_torrent_source_id BIGINT NOT NULL
        REFERENCES canonical_torrent_source (canonical_torrent_source_id)
        ON DELETE CASCADE,
    score_total_base NUMERIC(12, 4) NOT NULL
        CHECK (score_total_base BETWEEN -10000 AND 10000),
    score_seed NUMERIC(12, 4) NOT NULL,
    score_leech NUMERIC(12, 4) NOT NULL,
    score_age NUMERIC(12, 4) NOT NULL,
    score_trust NUMERIC(12, 4) NOT NULL,
    score_health NUMERIC(12, 4) NOT NULL,
    score_reputation NUMERIC(12, 4) NOT NULL,
    computed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT canonical_torrent_source_base_score_uq UNIQUE (
        canonical_torrent_id,
        canonical_torrent_source_id
    )
);

CREATE TABLE IF NOT EXISTS canonical_torrent_source_context_score (
    canonical_torrent_source_context_score_id BIGINT
        GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    context_key_type context_key_type NOT NULL,
    context_key_id BIGINT NOT NULL
        CHECK (context_key_id > 0),
    canonical_torrent_id BIGINT NOT NULL
        REFERENCES canonical_torrent (canonical_torrent_id) ON DELETE CASCADE,
    canonical_torrent_source_id BIGINT NOT NULL
        REFERENCES canonical_torrent_source (canonical_torrent_source_id)
        ON DELETE CASCADE,
    score_total_context NUMERIC(12, 4) NOT NULL
        CHECK (score_total_context BETWEEN -10000 AND 10000),
    score_policy_adjust NUMERIC(12, 4) NOT NULL,
    score_tag_adjust NUMERIC(12, 4) NOT NULL,
    is_dropped BOOLEAN NOT NULL DEFAULT FALSE,
    computed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT canonical_torrent_source_context_score_uq UNIQUE (
        context_key_type,
        context_key_id,
        canonical_torrent_id,
        canonical_torrent_source_id
    )
);

CREATE TABLE IF NOT EXISTS canonical_torrent_best_source_global (
    canonical_torrent_best_source_global_id BIGINT
        GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    canonical_torrent_id BIGINT NOT NULL
        REFERENCES canonical_torrent (canonical_torrent_id) ON DELETE CASCADE,
    canonical_torrent_source_id BIGINT NOT NULL
        REFERENCES canonical_torrent_source (canonical_torrent_source_id)
        ON DELETE CASCADE,
    computed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT canonical_torrent_best_source_global_uq UNIQUE (canonical_torrent_id)
);

CREATE TABLE IF NOT EXISTS canonical_torrent_best_source_context (
    canonical_torrent_best_source_context_id BIGINT
        GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    context_key_type context_key_type NOT NULL,
    context_key_id BIGINT NOT NULL
        CHECK (context_key_id > 0),
    canonical_torrent_id BIGINT NOT NULL
        REFERENCES canonical_torrent (canonical_torrent_id) ON DELETE CASCADE,
    canonical_torrent_source_id BIGINT NOT NULL
        REFERENCES canonical_torrent_source (canonical_torrent_source_id)
        ON DELETE CASCADE,
    computed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT canonical_torrent_best_source_context_uq UNIQUE (
        context_key_type,
        context_key_id,
        canonical_torrent_id
    )
);
