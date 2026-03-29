CREATE OR REPLACE FUNCTION indexer_backup_assert_actor_v1(actor_user_public_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to export indexer backup';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    actor_verified BOOLEAN;
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_missing';
    END IF;

    SELECT user_id, role, is_email_verified
    INTO actor_user_id, actor_role, actor_verified
    FROM app_user
    WHERE user_public_id = actor_user_public_id;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_not_found';
    END IF;

    IF actor_verified IS DISTINCT FROM TRUE THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_unverified';
    END IF;

    IF actor_role NOT IN ('owner', 'admin') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_unauthorized';
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_backup_export_tag_list_v1(actor_user_public_id UUID)
RETURNS TABLE (
    tag_public_id UUID,
    tag_key VARCHAR,
    display_name VARCHAR
)
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_backup_assert_actor_v1(actor_user_public_id);

    RETURN QUERY
    SELECT
        tag.tag_public_id,
        tag.tag_key,
        tag.display_name
    FROM tag
    WHERE tag.deleted_at IS NULL
    ORDER BY tag.tag_key;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_backup_export_tag_list(actor_user_public_id UUID)
RETURNS TABLE (
    tag_public_id UUID,
    tag_key VARCHAR,
    display_name VARCHAR
)
LANGUAGE sql
AS $$
    SELECT *
    FROM indexer_backup_export_tag_list_v1(actor_user_public_id);
$$;

CREATE OR REPLACE FUNCTION indexer_backup_export_rate_limit_policy_list_v1(
    actor_user_public_id UUID
)
RETURNS TABLE (
    rate_limit_policy_public_id UUID,
    display_name VARCHAR,
    requests_per_minute INTEGER,
    burst INTEGER,
    concurrent_requests INTEGER,
    is_system BOOLEAN
)
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_backup_assert_actor_v1(actor_user_public_id);

    RETURN QUERY
    SELECT
        policy.rate_limit_policy_public_id,
        policy.display_name,
        policy.requests_per_minute,
        policy.burst,
        policy.concurrent_requests,
        policy.is_system
    FROM rate_limit_policy policy
    WHERE policy.deleted_at IS NULL
    ORDER BY policy.display_name;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_backup_export_rate_limit_policy_list(
    actor_user_public_id UUID
)
RETURNS TABLE (
    rate_limit_policy_public_id UUID,
    display_name VARCHAR,
    requests_per_minute INTEGER,
    burst INTEGER,
    concurrent_requests INTEGER,
    is_system BOOLEAN
)
LANGUAGE sql
AS $$
    SELECT *
    FROM indexer_backup_export_rate_limit_policy_list_v1(actor_user_public_id);
$$;

CREATE OR REPLACE FUNCTION indexer_backup_export_routing_policy_list_v1(
    actor_user_public_id UUID
)
RETURNS TABLE (
    routing_policy_public_id UUID,
    display_name VARCHAR,
    mode routing_policy_mode,
    rate_limit_policy_public_id UUID,
    rate_limit_display_name VARCHAR,
    param_key routing_param_key,
    value_plain VARCHAR,
    value_int INTEGER,
    value_bool BOOLEAN,
    secret_public_id UUID,
    secret_type secret_type
)
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_backup_assert_actor_v1(actor_user_public_id);

    RETURN QUERY
    SELECT
        policy.routing_policy_public_id,
        policy.display_name,
        policy.mode,
        rate_limit.rate_limit_policy_public_id,
        rate_limit.display_name,
        param.param_key,
        param.value_plain,
        param.value_int,
        param.value_bool,
        secret.secret_public_id,
        secret.secret_type
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
    LEFT JOIN secret
        ON secret.secret_id = binding.secret_id
       AND secret.is_revoked = FALSE
    WHERE policy.deleted_at IS NULL
    ORDER BY policy.display_name, param.param_key NULLS FIRST;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_backup_export_routing_policy_list(
    actor_user_public_id UUID
)
RETURNS TABLE (
    routing_policy_public_id UUID,
    display_name VARCHAR,
    mode routing_policy_mode,
    rate_limit_policy_public_id UUID,
    rate_limit_display_name VARCHAR,
    param_key routing_param_key,
    value_plain VARCHAR,
    value_int INTEGER,
    value_bool BOOLEAN,
    secret_public_id UUID,
    secret_type secret_type
)
LANGUAGE sql
AS $$
    SELECT *
    FROM indexer_backup_export_routing_policy_list_v1(actor_user_public_id);
$$;

CREATE OR REPLACE FUNCTION indexer_backup_export_indexer_instance_list_v1(
    actor_user_public_id UUID
)
RETURNS TABLE (
    indexer_instance_public_id UUID,
    upstream_slug VARCHAR,
    display_name VARCHAR,
    is_enabled BOOLEAN,
    enable_rss BOOLEAN,
    enable_automatic_search BOOLEAN,
    enable_interactive_search BOOLEAN,
    priority INTEGER,
    trust_tier_key trust_tier_key,
    routing_policy_public_id UUID,
    routing_policy_display_name VARCHAR,
    connect_timeout_ms INTEGER,
    read_timeout_ms INTEGER,
    max_parallel_requests INTEGER,
    rate_limit_policy_public_id UUID,
    rate_limit_display_name VARCHAR,
    rss_subscription_enabled BOOLEAN,
    rss_interval_seconds INTEGER,
    media_domain_key media_domain_key,
    tag_key VARCHAR,
    field_name VARCHAR,
    field_type field_type,
    value_plain VARCHAR,
    value_int INTEGER,
    value_decimal VARCHAR,
    value_bool BOOLEAN,
    secret_public_id UUID,
    secret_type secret_type
)
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_backup_assert_actor_v1(actor_user_public_id);

    RETURN QUERY
    SELECT
        instance.indexer_instance_public_id,
        definition.upstream_slug,
        instance.display_name,
        instance.is_enabled,
        instance.enable_rss,
        instance.enable_automatic_search,
        instance.enable_interactive_search,
        instance.priority,
        instance.trust_tier_key,
        routing.routing_policy_public_id,
        routing.display_name,
        instance.connect_timeout_ms,
        instance.read_timeout_ms,
        instance.max_parallel_requests,
        rate_limit.rate_limit_policy_public_id,
        rate_limit.display_name,
        rss.is_enabled,
        rss.interval_seconds,
        media_domain.media_domain_key,
        tag.tag_key,
        field_value.field_name,
        field_value.field_type,
        field_value.value_plain,
        field_value.value_int,
        field_value.value_decimal::text,
        field_value.value_bool,
        secret.secret_public_id,
        secret.secret_type
    FROM indexer_instance instance
    JOIN indexer_definition definition
        ON definition.indexer_definition_id = instance.indexer_definition_id
    LEFT JOIN routing_policy routing
        ON routing.routing_policy_id = instance.routing_policy_id
       AND routing.deleted_at IS NULL
    LEFT JOIN indexer_instance_rate_limit instance_rate_limit
        ON instance_rate_limit.indexer_instance_id = instance.indexer_instance_id
    LEFT JOIN rate_limit_policy rate_limit
        ON rate_limit.rate_limit_policy_id = instance_rate_limit.rate_limit_policy_id
       AND rate_limit.deleted_at IS NULL
    LEFT JOIN indexer_rss_subscription rss
        ON rss.indexer_instance_id = instance.indexer_instance_id
    LEFT JOIN indexer_instance_media_domain instance_media_domain
        ON instance_media_domain.indexer_instance_id = instance.indexer_instance_id
    LEFT JOIN media_domain
        ON media_domain.media_domain_id = instance_media_domain.media_domain_id
    LEFT JOIN indexer_instance_tag instance_tag
        ON instance_tag.indexer_instance_id = instance.indexer_instance_id
    LEFT JOIN tag
        ON tag.tag_id = instance_tag.tag_id
       AND tag.deleted_at IS NULL
    LEFT JOIN indexer_instance_field_value field_value
        ON field_value.indexer_instance_id = instance.indexer_instance_id
    LEFT JOIN secret_binding binding
        ON binding.bound_table = 'indexer_instance_field_value'
       AND binding.bound_id = field_value.indexer_instance_field_value_id
    LEFT JOIN secret
        ON secret.secret_id = binding.secret_id
       AND secret.is_revoked = FALSE
    WHERE instance.deleted_at IS NULL
    ORDER BY instance.display_name, field_value.field_name NULLS FIRST;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_backup_export_indexer_instance_list(
    actor_user_public_id UUID
)
RETURNS TABLE (
    indexer_instance_public_id UUID,
    upstream_slug VARCHAR,
    display_name VARCHAR,
    is_enabled BOOLEAN,
    enable_rss BOOLEAN,
    enable_automatic_search BOOLEAN,
    enable_interactive_search BOOLEAN,
    priority INTEGER,
    trust_tier_key trust_tier_key,
    routing_policy_public_id UUID,
    routing_policy_display_name VARCHAR,
    connect_timeout_ms INTEGER,
    read_timeout_ms INTEGER,
    max_parallel_requests INTEGER,
    rate_limit_policy_public_id UUID,
    rate_limit_display_name VARCHAR,
    rss_subscription_enabled BOOLEAN,
    rss_interval_seconds INTEGER,
    media_domain_key media_domain_key,
    tag_key VARCHAR,
    field_name VARCHAR,
    field_type field_type,
    value_plain VARCHAR,
    value_int INTEGER,
    value_decimal VARCHAR,
    value_bool BOOLEAN,
    secret_public_id UUID,
    secret_type secret_type
)
LANGUAGE sql
AS $$
    SELECT *
    FROM indexer_backup_export_indexer_instance_list_v1(actor_user_public_id);
$$;
