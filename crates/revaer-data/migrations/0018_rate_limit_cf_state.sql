-- Rate limiting and Cloudflare state.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'rate_limit_scope') THEN
        CREATE TYPE rate_limit_scope AS ENUM ('indexer_instance', 'routing_policy');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'cf_state') THEN
        CREATE TYPE cf_state AS ENUM (
            'clear',
            'challenged',
            'solved',
            'banned',
            'cooldown'
        );
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS rate_limit_policy (
    rate_limit_policy_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    rate_limit_policy_public_id UUID NOT NULL,
    display_name VARCHAR(256) NOT NULL,
    requests_per_minute INTEGER NOT NULL
        CHECK (requests_per_minute BETWEEN 1 AND 6000),
    burst INTEGER NOT NULL
        CHECK (burst BETWEEN 0 AND 6000),
    concurrent_requests INTEGER NOT NULL
        CHECK (concurrent_requests BETWEEN 1 AND 64),
    is_system BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CONSTRAINT rate_limit_policy_public_id_uq UNIQUE (rate_limit_policy_public_id),
    CONSTRAINT rate_limit_policy_display_name_uq UNIQUE (display_name)
);

CREATE TABLE IF NOT EXISTS indexer_instance_rate_limit (
    indexer_instance_rate_limit_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id) ON DELETE CASCADE,
    rate_limit_policy_id BIGINT NOT NULL
        REFERENCES rate_limit_policy (rate_limit_policy_id),
    CONSTRAINT indexer_instance_rate_limit_uq UNIQUE (indexer_instance_id)
);

CREATE TABLE IF NOT EXISTS routing_policy_rate_limit (
    routing_policy_rate_limit_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    routing_policy_id BIGINT NOT NULL
        REFERENCES routing_policy (routing_policy_id) ON DELETE CASCADE,
    rate_limit_policy_id BIGINT NOT NULL
        REFERENCES rate_limit_policy (rate_limit_policy_id),
    CONSTRAINT routing_policy_rate_limit_uq UNIQUE (routing_policy_id)
);

CREATE TABLE IF NOT EXISTS rate_limit_state (
    rate_limit_state_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    scope_type rate_limit_scope NOT NULL,
    scope_id BIGINT NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    tokens_used INTEGER NOT NULL
        CHECK (tokens_used >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT rate_limit_state_uq UNIQUE (scope_type, scope_id, window_start)
);

CREATE TABLE IF NOT EXISTS indexer_cf_state (
    indexer_cf_state_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id) ON DELETE CASCADE,
    state cf_state NOT NULL,
    last_changed_at TIMESTAMPTZ NOT NULL,
    cf_session_id VARCHAR(256),
    cf_session_expires_at TIMESTAMPTZ,
    cooldown_until TIMESTAMPTZ,
    backoff_seconds INTEGER
        CHECK (backoff_seconds IS NULL OR backoff_seconds >= 0),
    consecutive_failures INTEGER NOT NULL DEFAULT 0
        CHECK (consecutive_failures >= 0),
    last_error_class error_class,
    CONSTRAINT indexer_cf_state_instance_uq UNIQUE (indexer_instance_id)
);
