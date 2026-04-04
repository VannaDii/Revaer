-- Connectivity snapshots and audit logs.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'health_event_type') THEN
        CREATE TYPE health_event_type AS ENUM ('identity_conflict');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'connectivity_status') THEN
        CREATE TYPE connectivity_status AS ENUM (
            'healthy',
            'degraded',
            'failing',
            'quarantined'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'audit_entity_type') THEN
        CREATE TYPE audit_entity_type AS ENUM (
            'indexer_instance',
            'indexer_instance_field_value',
            'routing_policy',
            'routing_policy_parameter',
            'policy_set',
            'policy_rule',
            'search_profile',
            'search_profile_rule',
            'tag',
            'canonical_disambiguation_rule',
            'torznab_instance',
            'rate_limit_policy',
            'tracker_category_mapping',
            'media_domain_to_torznab_category'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'audit_action') THEN
        CREATE TYPE audit_action AS ENUM (
            'create',
            'update',
            'enable',
            'disable',
            'soft_delete',
            'restore'
        );
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS indexer_connectivity_profile (
    indexer_instance_id BIGINT PRIMARY KEY
        REFERENCES indexer_instance (indexer_instance_id),
    status connectivity_status NOT NULL,
    error_class error_class,
    latency_p50_ms INTEGER
        CHECK (latency_p50_ms IS NULL OR latency_p50_ms >= 0),
    latency_p95_ms INTEGER
        CHECK (latency_p95_ms IS NULL OR latency_p95_ms >= 0),
    success_rate_1h NUMERIC(5, 4)
        CHECK (
            success_rate_1h IS NULL
            OR (success_rate_1h BETWEEN 0 AND 1)
        ),
    success_rate_24h NUMERIC(5, 4)
        CHECK (
            success_rate_24h IS NULL
            OR (success_rate_24h BETWEEN 0 AND 1)
        ),
    last_checked_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT indexer_connectivity_profile_error_class_chk CHECK (
        (status = 'healthy' AND error_class IS NULL)
        OR (status <> 'healthy')
    )
);

CREATE TABLE IF NOT EXISTS indexer_health_event (
    indexer_health_event_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id),
    occurred_at TIMESTAMPTZ NOT NULL,
    event_type health_event_type NOT NULL,
    latency_ms INTEGER
        CHECK (latency_ms IS NULL OR latency_ms >= 0),
    http_status INTEGER,
    error_class error_class,
    detail VARCHAR(1024)
);

CREATE TABLE IF NOT EXISTS config_audit_log (
    audit_log_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    entity_type audit_entity_type NOT NULL,
    entity_pk_bigint BIGINT,
    entity_public_id UUID,
    action audit_action NOT NULL,
    changed_by_user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    changed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    change_summary VARCHAR(1024) NOT NULL,
    change_detail VARCHAR(1024),
    CONSTRAINT config_audit_entity_ref_chk CHECK (
        entity_pk_bigint IS NOT NULL OR entity_public_id IS NOT NULL
    )
);
