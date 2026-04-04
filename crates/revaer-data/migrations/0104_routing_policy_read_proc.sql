-- Read procedures for routing policy operator visibility.

CREATE OR REPLACE FUNCTION routing_policy_get_v1(
    actor_user_public_id UUID,
    routing_policy_public_id_input UUID
)
RETURNS TABLE (
    routing_policy_public_id UUID,
    display_name VARCHAR,
    mode routing_policy_mode,
    rate_limit_policy_public_id UUID,
    rate_limit_display_name VARCHAR,
    rate_limit_requests_per_minute INTEGER,
    rate_limit_burst INTEGER,
    rate_limit_concurrent_requests INTEGER,
    param_key routing_param_key,
    value_plain VARCHAR,
    value_int INTEGER,
    value_bool BOOLEAN,
    secret_public_id UUID,
    secret_binding_name secret_binding_name
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to fetch routing policy';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    policy_id BIGINT;
    policy_deleted_at TIMESTAMPTZ;
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

    SELECT routing_policy_id, deleted_at
    INTO policy_id, policy_deleted_at
    FROM routing_policy
    WHERE routing_policy.routing_policy_public_id = routing_policy_public_id_input;

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

    RETURN QUERY
    SELECT
        policy.routing_policy_public_id,
        policy.display_name,
        policy.mode,
        rate_limit.rate_limit_policy_public_id,
        rate_limit.display_name,
        rate_limit.requests_per_minute,
        rate_limit.burst,
        rate_limit.concurrent_requests,
        param.param_key,
        param.value_plain,
        param.value_int,
        param.value_bool,
        secret.secret_public_id,
        binding.binding_name
    FROM routing_policy policy
    LEFT JOIN routing_policy_rate_limit policy_rate_limit
        ON policy_rate_limit.routing_policy_id = policy.routing_policy_id
    LEFT JOIN rate_limit_policy rate_limit
        ON rate_limit.rate_limit_policy_id = policy_rate_limit.rate_limit_policy_id
       AND rate_limit.deleted_at IS NULL
    LEFT JOIN routing_policy_parameter param
        ON param.routing_policy_id = policy.routing_policy_id
    LEFT JOIN secret_binding binding
        ON binding.bound_table = 'routing_policy_parameter'
       AND binding.bound_id = param.routing_policy_parameter_id
    LEFT JOIN secret secret
        ON secret.secret_id = binding.secret_id
       AND secret.is_revoked = FALSE
    WHERE policy.routing_policy_id = policy_id
    ORDER BY param.param_key NULLS LAST;
END;
$$;

CREATE OR REPLACE FUNCTION routing_policy_get(
    actor_user_public_id UUID,
    routing_policy_public_id_input UUID
)
RETURNS TABLE (
    routing_policy_public_id UUID,
    display_name VARCHAR,
    mode routing_policy_mode,
    rate_limit_policy_public_id UUID,
    rate_limit_display_name VARCHAR,
    rate_limit_requests_per_minute INTEGER,
    rate_limit_burst INTEGER,
    rate_limit_concurrent_requests INTEGER,
    param_key routing_param_key,
    value_plain VARCHAR,
    value_int INTEGER,
    value_bool BOOLEAN,
    secret_public_id UUID,
    secret_binding_name secret_binding_name
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT *
    FROM routing_policy_get_v1(
        actor_user_public_id,
        routing_policy_public_id_input
    );
END;
$$;
