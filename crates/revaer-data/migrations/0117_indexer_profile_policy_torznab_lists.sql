CREATE OR REPLACE FUNCTION indexer_search_profile_list_v1(actor_user_public_id UUID)
RETURNS TABLE (
    search_profile_public_id UUID,
    display_name VARCHAR,
    is_default BOOLEAN,
    page_size INTEGER,
    default_media_domain_key media_domain_key,
    media_domain_keys VARCHAR[],
    policy_set_public_ids UUID[],
    policy_set_display_names VARCHAR[],
    allow_indexer_public_ids UUID[],
    block_indexer_public_ids UUID[],
    allow_tag_keys VARCHAR[],
    block_tag_keys VARCHAR[],
    prefer_tag_keys VARCHAR[]
)
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_backup_assert_actor_v1(actor_user_public_id);

    RETURN QUERY
    SELECT
        profile.search_profile_public_id,
        profile.display_name,
        profile.is_default,
        profile.page_size,
        default_domain.media_domain_key,
        COALESCE((
            SELECT array_agg(domain.media_domain_key::VARCHAR ORDER BY domain.media_domain_key)
            FROM search_profile_media_domain mapping
            INNER JOIN media_domain domain
                ON domain.media_domain_id = mapping.media_domain_id
            WHERE mapping.search_profile_id = profile.search_profile_id
        ), ARRAY[]::VARCHAR[]),
        COALESCE((
            SELECT array_agg(policy.policy_set_public_id ORDER BY policy.display_name)
            FROM search_profile_policy_set mapping
            INNER JOIN policy_set policy
                ON policy.policy_set_id = mapping.policy_set_id
               AND policy.deleted_at IS NULL
            WHERE mapping.search_profile_id = profile.search_profile_id
        ), ARRAY[]::UUID[]),
        COALESCE((
            SELECT array_agg(policy.display_name ORDER BY policy.display_name)
            FROM search_profile_policy_set mapping
            INNER JOIN policy_set policy
                ON policy.policy_set_id = mapping.policy_set_id
               AND policy.deleted_at IS NULL
            WHERE mapping.search_profile_id = profile.search_profile_id
        ), ARRAY[]::VARCHAR[]),
        COALESCE((
            SELECT array_agg(instance.indexer_instance_public_id ORDER BY instance.display_name)
            FROM search_profile_indexer_allow mapping
            INNER JOIN indexer_instance instance
                ON instance.indexer_instance_id = mapping.indexer_instance_id
               AND instance.deleted_at IS NULL
            WHERE mapping.search_profile_id = profile.search_profile_id
        ), ARRAY[]::UUID[]),
        COALESCE((
            SELECT array_agg(instance.indexer_instance_public_id ORDER BY instance.display_name)
            FROM search_profile_indexer_block mapping
            INNER JOIN indexer_instance instance
                ON instance.indexer_instance_id = mapping.indexer_instance_id
               AND instance.deleted_at IS NULL
            WHERE mapping.search_profile_id = profile.search_profile_id
        ), ARRAY[]::UUID[]),
        COALESCE((
            SELECT array_agg(tag.tag_key ORDER BY tag.tag_key)
            FROM search_profile_tag_allow mapping
            INNER JOIN tag
                ON tag.tag_id = mapping.tag_id
               AND tag.deleted_at IS NULL
            WHERE mapping.search_profile_id = profile.search_profile_id
        ), ARRAY[]::VARCHAR[]),
        COALESCE((
            SELECT array_agg(tag.tag_key ORDER BY tag.tag_key)
            FROM search_profile_tag_block mapping
            INNER JOIN tag
                ON tag.tag_id = mapping.tag_id
               AND tag.deleted_at IS NULL
            WHERE mapping.search_profile_id = profile.search_profile_id
        ), ARRAY[]::VARCHAR[]),
        COALESCE((
            SELECT array_agg(tag.tag_key ORDER BY tag.tag_key)
            FROM search_profile_tag_prefer mapping
            INNER JOIN tag
                ON tag.tag_id = mapping.tag_id
               AND tag.deleted_at IS NULL
            WHERE mapping.search_profile_id = profile.search_profile_id
        ), ARRAY[]::VARCHAR[])
    FROM search_profile profile
    LEFT JOIN media_domain default_domain
        ON default_domain.media_domain_id = profile.default_media_domain_id
    WHERE profile.deleted_at IS NULL
    ORDER BY profile.display_name, profile.search_profile_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_search_profile_list(actor_user_public_id UUID)
RETURNS TABLE (
    search_profile_public_id UUID,
    display_name VARCHAR,
    is_default BOOLEAN,
    page_size INTEGER,
    default_media_domain_key media_domain_key,
    media_domain_keys VARCHAR[],
    policy_set_public_ids UUID[],
    policy_set_display_names VARCHAR[],
    allow_indexer_public_ids UUID[],
    block_indexer_public_ids UUID[],
    allow_tag_keys VARCHAR[],
    block_tag_keys VARCHAR[],
    prefer_tag_keys VARCHAR[]
)
LANGUAGE sql
AS $$
    SELECT *
    FROM indexer_search_profile_list_v1(actor_user_public_id);
$$;

CREATE OR REPLACE FUNCTION indexer_policy_set_rule_list_v1(actor_user_public_id UUID)
RETURNS TABLE (
    policy_set_public_id UUID,
    policy_set_display_name VARCHAR,
    scope policy_scope,
    is_enabled BOOLEAN,
    user_public_id UUID,
    policy_rule_public_id UUID,
    rule_type policy_rule_type,
    match_field policy_match_field,
    match_operator policy_match_operator,
    sort_order INTEGER,
    match_value_text VARCHAR,
    match_value_int INTEGER,
    match_value_uuid UUID,
    action policy_action,
    severity policy_severity,
    is_case_insensitive BOOLEAN,
    rationale VARCHAR,
    expires_at TIMESTAMPTZ,
    is_rule_disabled BOOLEAN
)
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_backup_assert_actor_v1(actor_user_public_id);

    RETURN QUERY
    SELECT
        policy.policy_set_public_id,
        policy.display_name,
        policy.scope,
        policy.is_enabled,
        owner.user_public_id,
        rule.policy_rule_public_id,
        rule.rule_type,
        rule.match_field,
        rule.match_operator,
        rule.sort_order,
        rule.match_value_text,
        rule.match_value_int,
        rule.match_value_uuid,
        rule.action,
        rule.severity,
        rule.is_case_insensitive,
        rule.rationale,
        rule.expires_at,
        rule.is_disabled
    FROM policy_set policy
    LEFT JOIN app_user owner
        ON owner.user_id = policy.user_id
    LEFT JOIN policy_rule rule
        ON rule.policy_set_id = policy.policy_set_id
    WHERE policy.deleted_at IS NULL
    ORDER BY
        policy.display_name,
        policy.policy_set_public_id,
        rule.sort_order NULLS FIRST,
        rule.policy_rule_public_id NULLS FIRST;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_policy_set_rule_list(actor_user_public_id UUID)
RETURNS TABLE (
    policy_set_public_id UUID,
    policy_set_display_name VARCHAR,
    scope policy_scope,
    is_enabled BOOLEAN,
    user_public_id UUID,
    policy_rule_public_id UUID,
    rule_type policy_rule_type,
    match_field policy_match_field,
    match_operator policy_match_operator,
    sort_order INTEGER,
    match_value_text VARCHAR,
    match_value_int INTEGER,
    match_value_uuid UUID,
    action policy_action,
    severity policy_severity,
    is_case_insensitive BOOLEAN,
    rationale VARCHAR,
    expires_at TIMESTAMPTZ,
    is_rule_disabled BOOLEAN
)
LANGUAGE sql
AS $$
    SELECT *
    FROM indexer_policy_set_rule_list_v1(actor_user_public_id);
$$;

CREATE OR REPLACE FUNCTION indexer_torznab_instance_list_v1(actor_user_public_id UUID)
RETURNS TABLE (
    torznab_instance_public_id UUID,
    display_name VARCHAR,
    is_enabled BOOLEAN,
    search_profile_public_id UUID,
    search_profile_display_name VARCHAR
)
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_backup_assert_actor_v1(actor_user_public_id);

    RETURN QUERY
    SELECT
        instance.torznab_instance_public_id,
        instance.display_name,
        instance.is_enabled,
        profile.search_profile_public_id,
        profile.display_name
    FROM torznab_instance instance
    INNER JOIN search_profile profile
        ON profile.search_profile_id = instance.search_profile_id
       AND profile.deleted_at IS NULL
    WHERE instance.deleted_at IS NULL
    ORDER BY instance.display_name, instance.torznab_instance_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_torznab_instance_list(actor_user_public_id UUID)
RETURNS TABLE (
    torznab_instance_public_id UUID,
    display_name VARCHAR,
    is_enabled BOOLEAN,
    search_profile_public_id UUID,
    search_profile_display_name VARCHAR
)
LANGUAGE sql
AS $$
    SELECT *
    FROM indexer_torznab_instance_list_v1(actor_user_public_id);
$$;
