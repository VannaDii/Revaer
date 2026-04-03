-- Operator-facing list reads for indexer tags and secret metadata.

CREATE OR REPLACE FUNCTION tag_list_v1(
    actor_user_public_id UUID
)
RETURNS TABLE (
    tag_public_id UUID,
    tag_key VARCHAR,
    display_name VARCHAR,
    updated_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to list tags';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_missing';
    END IF;

    SELECT user_id, role
    INTO actor_user_id, actor_role
    FROM app_user
    WHERE user_public_id = actor_user_public_id;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_not_found';
    END IF;

    IF actor_role NOT IN ('owner', 'admin') THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_unauthorized';
    END IF;

    RETURN QUERY
    SELECT
        tag.tag_public_id,
        tag.tag_key,
        tag.display_name,
        tag.updated_at
    FROM tag
    WHERE tag.deleted_at IS NULL
    ORDER BY tag.display_name ASC, tag.tag_id ASC;
END;
$$;

CREATE OR REPLACE FUNCTION tag_list(
    actor_user_public_id UUID
)
RETURNS TABLE (
    tag_public_id UUID,
    tag_key VARCHAR,
    display_name VARCHAR,
    updated_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT *
    FROM tag_list_v1(actor_user_public_id);
END;
$$;

CREATE OR REPLACE FUNCTION secret_metadata_list_v1(
    actor_user_public_id UUID
)
RETURNS TABLE (
    secret_public_id UUID,
    secret_type secret_type,
    is_revoked BOOLEAN,
    created_at TIMESTAMPTZ,
    rotated_at TIMESTAMPTZ,
    binding_count BIGINT
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to list secret metadata';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_missing';
    END IF;

    SELECT user_id, role
    INTO actor_user_id, actor_role
    FROM app_user
    WHERE user_public_id = actor_user_public_id;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_not_found';
    END IF;

    IF actor_role NOT IN ('owner', 'admin') THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_unauthorized';
    END IF;

    RETURN QUERY
    SELECT
        secret.secret_public_id,
        secret.secret_type,
        secret.is_revoked,
        secret.created_at,
        secret.rotated_at,
        COUNT(secret_binding.secret_binding_id) AS binding_count
    FROM secret
    LEFT JOIN secret_binding
        ON secret_binding.secret_id = secret.secret_id
    GROUP BY
        secret.secret_id,
        secret.secret_public_id,
        secret.secret_type,
        secret.is_revoked,
        secret.created_at,
        secret.rotated_at
    ORDER BY secret.created_at DESC, secret.secret_id DESC;
END;
$$;

CREATE OR REPLACE FUNCTION secret_metadata_list(
    actor_user_public_id UUID
)
RETURNS TABLE (
    secret_public_id UUID,
    secret_type secret_type,
    is_revoked BOOLEAN,
    created_at TIMESTAMPTZ,
    rotated_at TIMESTAMPTZ,
    binding_count BIGINT
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT *
    FROM secret_metadata_list_v1(actor_user_public_id);
END;
$$;
