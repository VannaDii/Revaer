-- User actions and acquisition attempts.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'user_action') THEN
        CREATE TYPE user_action AS ENUM (
            'viewed',
            'selected',
            'deselected',
            'downloaded',
            'blocked',
            'reported_fake',
            'preferred_source',
            'separated_canonical',
            'feedback_positive',
            'feedback_negative'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'user_reason_code') THEN
        CREATE TYPE user_reason_code AS ENUM (
            'none',
            'wrong_title',
            'wrong_language',
            'wrong_quality',
            'suspicious',
            'known_bad_group',
            'dmca_risk',
            'dead_torrent',
            'duplicate',
            'personal_preference',
            'other'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'user_action_kv_key') THEN
        CREATE TYPE user_action_kv_key AS ENUM (
            'ui_surface',
            'device',
            'chosen_indexer_instance_public_id',
            'chosen_source_public_id',
            'note_short'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'acquisition_status') THEN
        CREATE TYPE acquisition_status AS ENUM (
            'started',
            'succeeded',
            'failed',
            'canceled'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'acquisition_origin') THEN
        CREATE TYPE acquisition_origin AS ENUM (
            'torznab',
            'ui',
            'api',
            'automation'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'acquisition_failure_class') THEN
        CREATE TYPE acquisition_failure_class AS ENUM (
            'dead',
            'dmca',
            'passworded',
            'corrupted',
            'stalled',
            'not_enough_space',
            'auth_error',
            'network_error',
            'client_error',
            'user_canceled',
            'unknown'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'torrent_client_name') THEN
        CREATE TYPE torrent_client_name AS ENUM (
            'revaer_internal',
            'transmission',
            'qbittorrent',
            'deluge',
            'rtorrent',
            'aria2',
            'unknown'
        );
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS user_result_action (
    user_result_action_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    search_request_id BIGINT NOT NULL
        REFERENCES search_request (search_request_id),
    canonical_torrent_id BIGINT NOT NULL
        REFERENCES canonical_torrent (canonical_torrent_id),
    action user_action NOT NULL,
    reason_code user_reason_code NOT NULL,
    reason_text VARCHAR(512),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS user_result_action_kv (
    user_result_action_kv_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_result_action_id BIGINT NOT NULL
        REFERENCES user_result_action (user_result_action_id) ON DELETE CASCADE,
    key user_action_kv_key NOT NULL,
    value VARCHAR(512) NOT NULL,
    CONSTRAINT user_result_action_kv_uq UNIQUE (user_result_action_id, key)
);

CREATE TABLE IF NOT EXISTS acquisition_attempt (
    acquisition_attempt_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    torznab_instance_id BIGINT
        REFERENCES torznab_instance (torznab_instance_id),
    origin acquisition_origin NOT NULL,
    canonical_torrent_id BIGINT NOT NULL
        REFERENCES canonical_torrent (canonical_torrent_id),
    canonical_torrent_source_id BIGINT NOT NULL
        REFERENCES canonical_torrent_source (canonical_torrent_source_id),
    search_request_id BIGINT
        REFERENCES search_request (search_request_id),
    user_id BIGINT
        REFERENCES app_user (user_id),
    infohash_v1 CHAR(40),
    infohash_v2 CHAR(64),
    magnet_hash CHAR(64),
    torrent_client_id VARCHAR(128),
    torrent_client_name torrent_client_name NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ,
    status acquisition_status NOT NULL,
    failure_class acquisition_failure_class,
    failure_detail VARCHAR(256),
    CONSTRAINT acquisition_attempt_identifier_chk CHECK (
        infohash_v1 IS NOT NULL
        OR infohash_v2 IS NOT NULL
        OR magnet_hash IS NOT NULL
    ),
    CONSTRAINT acquisition_attempt_infohash_v1_chk CHECK (
        infohash_v1 IS NULL OR infohash_v1 ~ '^[0-9a-f]{40}$'
    ),
    CONSTRAINT acquisition_attempt_infohash_v2_chk CHECK (
        infohash_v2 IS NULL OR infohash_v2 ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT acquisition_attempt_magnet_hash_chk CHECK (
        magnet_hash IS NULL OR magnet_hash ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT acquisition_attempt_failure_class_chk CHECK (
        (status = 'failed' AND failure_class IS NOT NULL)
        OR (status <> 'failed' AND failure_class IS NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS acquisition_attempt_client_uq
    ON acquisition_attempt (torrent_client_name, torrent_client_id)
    WHERE torrent_client_id IS NOT NULL
        AND torrent_client_name <> 'unknown';
