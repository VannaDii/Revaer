-- Source metadata conflict read procedure for operator review.

CREATE OR REPLACE FUNCTION source_metadata_conflict_list_v1(
    actor_user_public_id UUID,
    include_resolved_input BOOLEAN,
    limit_input INTEGER
)
RETURNS TABLE(
    conflict_id BIGINT,
    conflict_type conflict_type,
    existing_value VARCHAR,
    incoming_value VARCHAR,
    observed_at TIMESTAMPTZ,
    resolved_at TIMESTAMPTZ,
    resolution conflict_resolution,
    resolution_note VARCHAR
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to list source metadata conflicts';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    include_resolved_value BOOLEAN;
    row_limit INTEGER;
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

    include_resolved_value := COALESCE(include_resolved_input, FALSE);
    row_limit := COALESCE(limit_input, 50);

    IF row_limit < 1 OR row_limit > 200 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'limit_invalid';
    END IF;

    RETURN QUERY
    SELECT source_metadata_conflict.source_metadata_conflict_id,
           source_metadata_conflict.conflict_type,
           source_metadata_conflict.existing_value,
           source_metadata_conflict.incoming_value,
           source_metadata_conflict.observed_at,
           source_metadata_conflict.resolved_at,
           source_metadata_conflict.resolution,
           source_metadata_conflict.resolution_note
    FROM source_metadata_conflict
    WHERE include_resolved_value = TRUE
       OR source_metadata_conflict.resolved_at IS NULL
    ORDER BY source_metadata_conflict.observed_at DESC,
             source_metadata_conflict.source_metadata_conflict_id DESC
    LIMIT row_limit;
END;
$$;

CREATE OR REPLACE FUNCTION source_metadata_conflict_list(
    actor_user_public_id UUID,
    include_resolved_input BOOLEAN,
    limit_input INTEGER
)
RETURNS TABLE(
    conflict_id BIGINT,
    conflict_type conflict_type,
    existing_value VARCHAR,
    incoming_value VARCHAR,
    observed_at TIMESTAMPTZ,
    resolved_at TIMESTAMPTZ,
    resolution conflict_resolution,
    resolution_note VARCHAR
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT * FROM source_metadata_conflict_list_v1(
        actor_user_public_id => actor_user_public_id,
        include_resolved_input => include_resolved_input,
        limit_input => limit_input
    );
END;
$$;
