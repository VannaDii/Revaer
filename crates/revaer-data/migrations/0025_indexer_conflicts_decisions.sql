-- Conflict tracking and search filter decisions.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'conflict_type') THEN
        CREATE TYPE conflict_type AS ENUM (
            'tracker_name',
            'tracker_category',
            'external_id',
            'hash',
            'source_guid'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'conflict_resolution') THEN
        CREATE TYPE conflict_resolution AS ENUM (
            'accepted_incoming',
            'kept_existing',
            'merged',
            'ignored'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'source_metadata_conflict_action') THEN
        CREATE TYPE source_metadata_conflict_action AS ENUM (
            'created',
            'resolved',
            'reopened',
            'ignored'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'decision_type') THEN
        CREATE TYPE decision_type AS ENUM (
            'drop_canonical',
            'drop_source',
            'downrank',
            'flag'
        );
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS source_metadata_conflict (
    source_metadata_conflict_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    canonical_torrent_source_id BIGINT NOT NULL
        REFERENCES canonical_torrent_source (canonical_torrent_source_id)
        ON DELETE CASCADE,
    conflict_type conflict_type NOT NULL,
    existing_value VARCHAR(256) NOT NULL,
    incoming_value VARCHAR(256) NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    resolved_at TIMESTAMPTZ,
    resolved_by_user_id BIGINT
        REFERENCES app_user (user_id),
    resolution conflict_resolution,
    resolution_note VARCHAR(256)
);

CREATE TABLE IF NOT EXISTS source_metadata_conflict_audit_log (
    source_metadata_conflict_audit_log_id BIGINT
        GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    conflict_id BIGINT NOT NULL
        REFERENCES source_metadata_conflict (source_metadata_conflict_id)
        ON DELETE CASCADE,
    action source_metadata_conflict_action NOT NULL,
    actor_user_id BIGINT
        REFERENCES app_user (user_id),
    occurred_at TIMESTAMPTZ NOT NULL,
    note VARCHAR(256)
);

CREATE TABLE IF NOT EXISTS search_filter_decision (
    search_filter_decision_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    search_request_id BIGINT NOT NULL
        REFERENCES search_request (search_request_id) ON DELETE CASCADE,
    policy_rule_public_id UUID NOT NULL,
    policy_snapshot_id BIGINT NOT NULL
        REFERENCES policy_snapshot (policy_snapshot_id),
    observation_id BIGINT
        REFERENCES search_request_source_observation (observation_id),
    canonical_torrent_id BIGINT
        REFERENCES canonical_torrent (canonical_torrent_id),
    canonical_torrent_source_id BIGINT
        REFERENCES canonical_torrent_source (canonical_torrent_source_id),
    decision decision_type NOT NULL,
    decision_detail VARCHAR(512),
    decided_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT search_filter_decision_target_chk CHECK (
        canonical_torrent_id IS NOT NULL
        OR canonical_torrent_source_id IS NOT NULL
    )
);
