-- Indexer secrets and bindings.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'secret_type') THEN
        CREATE TYPE secret_type AS ENUM (
            'api_key',
            'password',
            'cookie',
            'token',
            'header_value'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'secret_bound_table') THEN
        CREATE TYPE secret_bound_table AS ENUM (
            'indexer_instance_field_value',
            'routing_policy_parameter'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'secret_binding_name') THEN
        CREATE TYPE secret_binding_name AS ENUM (
            'api_key',
            'password',
            'cookie',
            'token',
            'header_value',
            'proxy_password',
            'socks_password'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'secret_audit_action') THEN
        CREATE TYPE secret_audit_action AS ENUM (
            'create',
            'rotate',
            'revoke',
            'bind',
            'unbind'
        );
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS secret (
    secret_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    secret_public_id UUID NOT NULL,
    secret_type secret_type NOT NULL,
    cipher_text BYTEA NOT NULL,
    key_id VARCHAR(128) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    rotated_at TIMESTAMPTZ,
    is_revoked BOOLEAN NOT NULL DEFAULT FALSE,
    CONSTRAINT secret_public_id_uq UNIQUE (secret_public_id),
    CONSTRAINT secret_key_id_len_chk CHECK (char_length(key_id) BETWEEN 1 AND 128)
);

CREATE TABLE IF NOT EXISTS secret_binding (
    secret_binding_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    secret_id BIGINT NOT NULL
        REFERENCES secret (secret_id) ON DELETE RESTRICT,
    bound_table secret_bound_table NOT NULL,
    bound_id BIGINT NOT NULL,
    binding_name secret_binding_name NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT secret_binding_uq UNIQUE (bound_table, bound_id, binding_name),
    CONSTRAINT secret_binding_name_chk CHECK (
        (
            bound_table = 'indexer_instance_field_value'
            AND binding_name IN (
                'api_key',
                'password',
                'cookie',
                'token',
                'header_value'
            )
        )
        OR (
            bound_table = 'routing_policy_parameter'
            AND binding_name IN ('proxy_password', 'socks_password')
        )
    )
);

CREATE TABLE IF NOT EXISTS secret_audit_log (
    secret_audit_log_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    secret_id BIGINT NOT NULL
        REFERENCES secret (secret_id) ON DELETE RESTRICT,
    action secret_audit_action NOT NULL,
    actor_user_id BIGINT
        REFERENCES app_user (user_id),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    detail VARCHAR(256) NOT NULL,
    CONSTRAINT secret_audit_detail_len_chk CHECK (
        char_length(detail) BETWEEN 1 AND 256
    )
);
