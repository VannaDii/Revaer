-- Stored procedures for routing policy management.

CREATE OR REPLACE FUNCTION routing_policy_create_v1(
    actor_user_public_id UUID,
    display_name_input VARCHAR,
    mode_input routing_policy_mode
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to create routing policy';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    trimmed_display_name VARCHAR(256);
    new_policy_id BIGINT;
    new_policy_public_id UUID;
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_missing';
    END IF;

    SELECT user_id, role
    INTO actor_user_id, actor_role
    FROM app_user
    WHERE user_public_id = actor_user_public_id;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_not_found';
    END IF;

    IF actor_role NOT IN ('owner', 'admin') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_unauthorized';
    END IF;

    IF display_name_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'display_name_missing';
    END IF;

    trimmed_display_name := trim(display_name_input);

    IF trimmed_display_name = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'display_name_empty';
    END IF;

    IF char_length(trimmed_display_name) > 256 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'display_name_too_long';
    END IF;

    IF mode_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'mode_missing';
    END IF;

    IF mode_input IN ('vpn_route', 'tor') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'unsupported_routing_mode';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM routing_policy
        WHERE display_name = trimmed_display_name
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'display_name_already_exists';
    END IF;

    new_policy_public_id := gen_random_uuid();

    INSERT INTO routing_policy (
        routing_policy_public_id,
        display_name,
        mode,
        created_by_user_id,
        updated_by_user_id
    )
    VALUES (
        new_policy_public_id,
        trimmed_display_name,
        mode_input,
        actor_user_id,
        actor_user_id
    )
    RETURNING routing_policy_id INTO new_policy_id;

    INSERT INTO routing_policy_parameter (
        routing_policy_id,
        param_key,
        value_bool
    )
    VALUES (
        new_policy_id,
        'verify_tls',
        TRUE
    );

    IF mode_input = 'http_proxy' THEN
        INSERT INTO routing_policy_parameter (
            routing_policy_id,
            param_key
        )
        VALUES (
            new_policy_id,
            'http_proxy_auth'
        );
    ELSIF mode_input = 'socks_proxy' THEN
        INSERT INTO routing_policy_parameter (
            routing_policy_id,
            param_key
        )
        VALUES (
            new_policy_id,
            'socks_proxy_auth'
        );
    END IF;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'routing_policy',
        new_policy_id,
        new_policy_public_id,
        'create',
        actor_user_id,
        'routing_policy_create'
    );

    RETURN new_policy_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION routing_policy_create(
    actor_user_public_id UUID,
    display_name_input VARCHAR,
    mode_input routing_policy_mode
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN routing_policy_create_v1(actor_user_public_id, display_name_input, mode_input);
END;
$$;

CREATE OR REPLACE FUNCTION routing_policy_set_param_v1(
    actor_user_public_id UUID,
    routing_policy_public_id_input UUID,
    param_key_input routing_param_key,
    value_plain_input VARCHAR,
    value_int_input INTEGER,
    value_bool_input BOOLEAN
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to update routing policy parameter';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    policy_id BIGINT;
    policy_mode routing_policy_mode;
    policy_deleted_at TIMESTAMPTZ;
    is_param_allowed BOOLEAN;
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_missing';
    END IF;

    SELECT user_id, role
    INTO actor_user_id, actor_role
    FROM app_user
    WHERE user_public_id = actor_user_public_id;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_not_found';
    END IF;

    IF actor_role NOT IN ('owner', 'admin') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_unauthorized';
    END IF;

    IF routing_policy_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'routing_policy_missing';
    END IF;

    IF param_key_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'param_key_missing';
    END IF;

    SELECT routing_policy_id, mode, deleted_at
    INTO policy_id, policy_mode, policy_deleted_at
    FROM routing_policy
    WHERE routing_policy_public_id = routing_policy_public_id_input;

    IF policy_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'routing_policy_not_found';
    END IF;

    IF policy_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'routing_policy_deleted';
    END IF;

    is_param_allowed := CASE policy_mode
        WHEN 'direct' THEN param_key_input IN ('verify_tls')
        WHEN 'http_proxy' THEN param_key_input IN (
            'verify_tls',
            'proxy_host',
            'proxy_port',
            'proxy_username',
            'proxy_use_tls',
            'http_proxy_auth'
        )
        WHEN 'socks_proxy' THEN param_key_input IN (
            'verify_tls',
            'socks_host',
            'socks_port',
            'socks_username',
            'socks_proxy_auth'
        )
        WHEN 'flaresolverr' THEN param_key_input IN (
            'verify_tls',
            'fs_url',
            'fs_timeout_ms',
            'fs_session_ttl_seconds',
            'fs_user_agent'
        )
        ELSE FALSE
    END;

    IF is_param_allowed IS DISTINCT FROM TRUE THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'param_not_allowed';
    END IF;

    IF param_key_input IN ('http_proxy_auth', 'socks_proxy_auth') THEN
        IF value_plain_input IS NOT NULL
            OR value_int_input IS NOT NULL
            OR value_bool_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'param_requires_secret';
        END IF;

        INSERT INTO routing_policy_parameter (
            routing_policy_id,
            param_key
        )
        VALUES (
            policy_id,
            param_key_input
        )
        ON CONFLICT (routing_policy_id, param_key)
        DO NOTHING;
    ELSIF param_key_input IN ('verify_tls', 'proxy_use_tls') THEN
        IF value_bool_input IS NULL
            OR value_plain_input IS NOT NULL
            OR value_int_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'param_value_invalid';
        END IF;

        INSERT INTO routing_policy_parameter (
            routing_policy_id,
            param_key,
            value_bool
        )
        VALUES (
            policy_id,
            param_key_input,
            value_bool_input
        )
        ON CONFLICT (routing_policy_id, param_key)
        DO UPDATE SET value_bool = EXCLUDED.value_bool;
    ELSIF param_key_input IN ('proxy_port', 'socks_port') THEN
        IF value_int_input IS NULL
            OR value_plain_input IS NOT NULL
            OR value_bool_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'param_value_invalid';
        END IF;

        IF value_int_input < 1 OR value_int_input > 65535 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'param_value_out_of_range';
        END IF;

        INSERT INTO routing_policy_parameter (
            routing_policy_id,
            param_key,
            value_int
        )
        VALUES (
            policy_id,
            param_key_input,
            value_int_input
        )
        ON CONFLICT (routing_policy_id, param_key)
        DO UPDATE SET value_int = EXCLUDED.value_int;
    ELSIF param_key_input IN ('fs_timeout_ms') THEN
        IF value_int_input IS NULL
            OR value_plain_input IS NOT NULL
            OR value_bool_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'param_value_invalid';
        END IF;

        IF value_int_input < 1000 OR value_int_input > 300000 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'param_value_out_of_range';
        END IF;

        INSERT INTO routing_policy_parameter (
            routing_policy_id,
            param_key,
            value_int
        )
        VALUES (
            policy_id,
            param_key_input,
            value_int_input
        )
        ON CONFLICT (routing_policy_id, param_key)
        DO UPDATE SET value_int = EXCLUDED.value_int;
    ELSIF param_key_input IN ('fs_session_ttl_seconds') THEN
        IF value_int_input IS NULL
            OR value_plain_input IS NOT NULL
            OR value_bool_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'param_value_invalid';
        END IF;

        IF value_int_input < 60 OR value_int_input > 86400 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'param_value_out_of_range';
        END IF;

        INSERT INTO routing_policy_parameter (
            routing_policy_id,
            param_key,
            value_int
        )
        VALUES (
            policy_id,
            param_key_input,
            value_int_input
        )
        ON CONFLICT (routing_policy_id, param_key)
        DO UPDATE SET value_int = EXCLUDED.value_int;
    ELSE
        IF value_plain_input IS NULL
            OR value_int_input IS NOT NULL
            OR value_bool_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'param_value_invalid';
        END IF;

        IF char_length(value_plain_input) > 2048 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'param_value_too_long';
        END IF;

        INSERT INTO routing_policy_parameter (
            routing_policy_id,
            param_key,
            value_plain
        )
        VALUES (
            policy_id,
            param_key_input,
            value_plain_input
        )
        ON CONFLICT (routing_policy_id, param_key)
        DO UPDATE SET value_plain = EXCLUDED.value_plain;
    END IF;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'routing_policy',
        policy_id,
        routing_policy_public_id_input,
        'update',
        actor_user_id,
        'routing_policy_param_set'
    );
END;
$$;

CREATE OR REPLACE FUNCTION routing_policy_set_param(
    actor_user_public_id UUID,
    routing_policy_public_id_input UUID,
    param_key_input routing_param_key,
    value_plain_input VARCHAR,
    value_int_input INTEGER,
    value_bool_input BOOLEAN
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM routing_policy_set_param_v1(
        actor_user_public_id,
        routing_policy_public_id_input,
        param_key_input,
        value_plain_input,
        value_int_input,
        value_bool_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION routing_policy_bind_secret_v1(
    actor_user_public_id UUID,
    routing_policy_public_id_input UUID,
    param_key_input routing_param_key,
    secret_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to bind routing policy secret';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    policy_id BIGINT;
    policy_mode routing_policy_mode;
    policy_deleted_at TIMESTAMPTZ;
    param_id BIGINT;
    secret_id_value BIGINT;
    binding_name_value secret_binding_name;
    is_param_allowed BOOLEAN;
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_missing';
    END IF;

    SELECT user_id, role
    INTO actor_user_id, actor_role
    FROM app_user
    WHERE user_public_id = actor_user_public_id;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_not_found';
    END IF;

    IF actor_role NOT IN ('owner', 'admin') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_unauthorized';
    END IF;

    IF routing_policy_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'routing_policy_missing';
    END IF;

    IF param_key_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'param_key_missing';
    END IF;

    IF secret_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_missing';
    END IF;

    SELECT routing_policy_id, mode, deleted_at
    INTO policy_id, policy_mode, policy_deleted_at
    FROM routing_policy
    WHERE routing_policy_public_id = routing_policy_public_id_input;

    IF policy_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'routing_policy_not_found';
    END IF;

    IF policy_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'routing_policy_deleted';
    END IF;

    is_param_allowed := CASE policy_mode
        WHEN 'http_proxy' THEN param_key_input = 'http_proxy_auth'
        WHEN 'socks_proxy' THEN param_key_input = 'socks_proxy_auth'
        ELSE FALSE
    END;

    IF is_param_allowed IS DISTINCT FROM TRUE THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'param_not_allowed';
    END IF;

    SELECT secret_id
    INTO secret_id_value
    FROM secret
    WHERE secret_public_id = secret_public_id_input
      AND is_revoked = FALSE;

    IF secret_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_not_found';
    END IF;

    INSERT INTO routing_policy_parameter (
        routing_policy_id,
        param_key
    )
    VALUES (
        policy_id,
        param_key_input
    )
    ON CONFLICT (routing_policy_id, param_key)
    DO NOTHING;

    SELECT routing_policy_parameter_id
    INTO param_id
    FROM routing_policy_parameter
    WHERE routing_policy_id = policy_id
      AND param_key = param_key_input;

    binding_name_value := CASE param_key_input
        WHEN 'http_proxy_auth' THEN 'proxy_password'
        WHEN 'socks_proxy_auth' THEN 'socks_password'
        ELSE 'proxy_password'
    END;

    DELETE FROM secret_binding
    WHERE bound_table = 'routing_policy_parameter'
      AND bound_id = param_id
      AND binding_name = binding_name_value;

    INSERT INTO secret_binding (
        secret_id,
        bound_table,
        bound_id,
        binding_name
    )
    VALUES (
        secret_id_value,
        'routing_policy_parameter',
        param_id,
        binding_name_value
    );

    INSERT INTO secret_audit_log (
        secret_id,
        action,
        actor_user_id,
        detail
    )
    VALUES (
        secret_id_value,
        'bind',
        actor_user_id,
        'routing_policy_bind'
    );

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'routing_policy',
        policy_id,
        routing_policy_public_id_input,
        'update',
        actor_user_id,
        'routing_policy_bind_secret'
    );
END;
$$;

CREATE OR REPLACE FUNCTION routing_policy_bind_secret(
    actor_user_public_id UUID,
    routing_policy_public_id_input UUID,
    param_key_input routing_param_key,
    secret_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM routing_policy_bind_secret_v1(
        actor_user_public_id,
        routing_policy_public_id_input,
        param_key_input,
        secret_public_id_input
    );
END;
$$;
