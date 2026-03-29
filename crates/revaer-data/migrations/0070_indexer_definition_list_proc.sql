-- Indexer definition listing procedure.

CREATE OR REPLACE FUNCTION indexer_definition_list_v1(
    actor_user_public_id UUID
)
RETURNS TABLE (
    upstream_source VARCHAR,
    upstream_slug VARCHAR,
    display_name VARCHAR,
    protocol VARCHAR,
    engine VARCHAR,
    schema_version INTEGER,
    definition_hash CHAR(64),
    is_deprecated BOOLEAN,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to list indexer definitions';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_missing';
    END IF;

    SELECT user_id
    INTO actor_user_id
    FROM app_user
    WHERE user_public_id = actor_user_public_id;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_not_found';
    END IF;

    RETURN QUERY
    SELECT indexer_definition.upstream_source::text,
           indexer_definition.upstream_slug,
           indexer_definition.display_name,
           indexer_definition.protocol::text,
           indexer_definition.engine::text,
           indexer_definition.schema_version,
           indexer_definition.definition_hash,
           indexer_definition.is_deprecated,
           indexer_definition.created_at,
           indexer_definition.updated_at
    FROM indexer_definition
    ORDER BY indexer_definition.display_name ASC,
             indexer_definition.indexer_definition_id ASC;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_definition_list(
    actor_user_public_id UUID
)
RETURNS TABLE (
    upstream_source VARCHAR,
    upstream_slug VARCHAR,
    display_name VARCHAR,
    protocol VARCHAR,
    engine VARCHAR,
    schema_version INTEGER,
    definition_hash CHAR(64),
    is_deprecated BOOLEAN,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT * FROM indexer_definition_list_v1(actor_user_public_id);
END;
$$;
