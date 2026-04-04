CREATE OR REPLACE FUNCTION search_profile_set_domain_allowlist_v1(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    media_domain_keys_input TEXT[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to set domain allowlist';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    normalized_keys TEXT[];
    input_count INTEGER;
    resolved_count INTEGER;
    profile_default_media_domain_id BIGINT;
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

    IF search_profile_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_profile_missing';
    END IF;

    SELECT sp.search_profile_id, sp.user_id, sp.deleted_at, sp.default_media_domain_id
    INTO profile_id, profile_user_id, profile_deleted_at, profile_default_media_domain_id
    FROM search_profile sp
    WHERE sp.search_profile_public_id = search_profile_public_id_input;

    IF profile_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_profile_not_found';
    END IF;

    IF profile_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_profile_deleted';
    END IF;

    IF profile_user_id IS NULL THEN
        IF actor_role NOT IN ('owner', 'admin') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_unauthorized';
        END IF;
    ELSE
        IF profile_user_id <> actor_user_id AND actor_role NOT IN ('owner', 'admin') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_unauthorized';
        END IF;
    END IF;

    IF media_domain_keys_input IS NULL THEN
        normalized_keys := ARRAY[]::TEXT[];
    ELSE
        IF EXISTS (
            SELECT 1
            FROM unnest(media_domain_keys_input) AS value
            WHERE trim(value) = '' OR trim(value) <> lower(trim(value))
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'media_domain_key_invalid';
        END IF;

        SELECT array_agg(DISTINCT lower(trim(value)))
        INTO normalized_keys
        FROM unnest(media_domain_keys_input) AS value;
    END IF;

    IF normalized_keys IS NULL THEN
        normalized_keys := ARRAY[]::TEXT[];
    END IF;

    SELECT count(*)
    INTO input_count
    FROM unnest(normalized_keys) AS value
    WHERE value IS NOT NULL AND value <> '';

    IF input_count = 0 THEN
        DELETE FROM search_profile_media_domain
        WHERE search_profile_id = profile_id;
    ELSE
        SELECT count(*)
        INTO resolved_count
        FROM media_domain
        WHERE media_domain_key::TEXT = ANY(normalized_keys);

        IF resolved_count <> input_count THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'unknown_key';
        END IF;

        IF profile_default_media_domain_id IS NOT NULL THEN
            IF NOT EXISTS (
                SELECT 1
                FROM media_domain
                WHERE media_domain_id = profile_default_media_domain_id
                  AND media_domain_key::TEXT = ANY(normalized_keys)
            ) THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'default_not_in_allowlist';
            END IF;
        END IF;

        DELETE FROM search_profile_media_domain
        WHERE search_profile_id = profile_id;

        INSERT INTO search_profile_media_domain (
            search_profile_id,
            media_domain_id
        )
        SELECT profile_id, media_domain_id
        FROM media_domain
        WHERE media_domain_key::TEXT = ANY(normalized_keys);
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
        'search_profile_rule',
        NULL,
        search_profile_public_id_input,
        'update',
        actor_user_id,
        'search_profile_domain_allowlist'
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_set_domain_allowlist(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    media_domain_keys_input TEXT[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_profile_set_domain_allowlist_v1(
        actor_user_public_id => actor_user_public_id,
        search_profile_public_id_input => search_profile_public_id_input,
        media_domain_keys_input => media_domain_keys_input
    );
END;
$$;
