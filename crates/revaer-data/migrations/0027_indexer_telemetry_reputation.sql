-- Outbound request telemetry and reputation rollups.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'outbound_request_type') THEN
        CREATE TYPE outbound_request_type AS ENUM (
            'caps',
            'search',
            'tvsearch',
            'moviesearch',
            'rss',
            'probe'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'outbound_request_outcome') THEN
        CREATE TYPE outbound_request_outcome AS ENUM ('success', 'failure');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'outbound_via_mitigation') THEN
        CREATE TYPE outbound_via_mitigation AS ENUM (
            'none',
            'proxy',
            'flaresolverr'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'reputation_window') THEN
        CREATE TYPE reputation_window AS ENUM ('1h', '24h', '7d');
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS outbound_request_log (
    outbound_request_log_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id),
    routing_policy_id BIGINT
        REFERENCES routing_policy (routing_policy_id),
    search_request_id BIGINT
        REFERENCES search_request (search_request_id),
    request_type outbound_request_type NOT NULL,
    correlation_id UUID NOT NULL,
    retry_seq SMALLINT NOT NULL
        CHECK (retry_seq >= 0),
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ NOT NULL,
    outcome outbound_request_outcome NOT NULL,
    via_mitigation outbound_via_mitigation NOT NULL,
    rate_limit_denied_scope rate_limit_scope,
    error_class error_class,
    http_status INTEGER,
    latency_ms INTEGER,
    parse_ok BOOLEAN NOT NULL DEFAULT FALSE,
    result_count INTEGER,
    cf_detected BOOLEAN NOT NULL DEFAULT FALSE,
    page_number INTEGER,
    page_cursor_key VARCHAR(64),
    page_cursor_is_hashed BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT outbound_request_log_outcome_chk CHECK (
        (outcome = 'success' AND error_class IS NULL AND parse_ok = TRUE)
        OR (outcome = 'failure' AND error_class IS NOT NULL)
    ),
    CONSTRAINT outbound_request_log_rate_limit_scope_chk CHECK (
        (rate_limit_denied_scope IS NULL AND error_class IS DISTINCT FROM 'rate_limited')
        OR (rate_limit_denied_scope IS NOT NULL AND error_class = 'rate_limited')
    ),
    CONSTRAINT outbound_request_log_result_count_chk CHECK (
        result_count IS NULL OR result_count >= 0
    ),
    CONSTRAINT outbound_request_log_page_number_chk CHECK (
        page_number IS NULL OR page_number >= 1
    ),
    CONSTRAINT outbound_request_log_latency_chk CHECK (
        latency_ms IS NULL OR latency_ms >= 0
    )
);

CREATE TABLE IF NOT EXISTS source_reputation (
    source_reputation_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_instance_id BIGINT NOT NULL
        REFERENCES indexer_instance (indexer_instance_id) ON DELETE CASCADE,
    window_key reputation_window NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    request_success_rate NUMERIC(5, 4) NOT NULL
        CHECK (request_success_rate BETWEEN 0 AND 1),
    acquisition_success_rate NUMERIC(5, 4) NOT NULL
        CHECK (acquisition_success_rate BETWEEN 0 AND 1),
    fake_rate NUMERIC(5, 4) NOT NULL
        CHECK (fake_rate BETWEEN 0 AND 1),
    dmca_rate NUMERIC(5, 4) NOT NULL
        CHECK (dmca_rate BETWEEN 0 AND 1),
    request_count INTEGER NOT NULL
        CHECK (request_count >= 0),
    request_success_count INTEGER NOT NULL
        CHECK (request_success_count >= 0),
    acquisition_count INTEGER NOT NULL
        CHECK (acquisition_count >= 0),
    acquisition_success_count INTEGER NOT NULL
        CHECK (acquisition_success_count >= 0),
    min_samples INTEGER NOT NULL
        CHECK (min_samples >= 0),
    computed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT source_reputation_uq UNIQUE (
        indexer_instance_id,
        window_key,
        window_start
    )
);
