-- Stored procedures for search profile management.

CREATE OR REPLACE FUNCTION search_profile_create_v1(
    actor_user_public_id UUID,
    display_name_input VARCHAR,
    is_default_input BOOLEAN,
    page_size_input INTEGER,
    default_media_domain_key_input VARCHAR,
    user_public_id_input UUID
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to create search profile';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    target_user_id BIGINT;
    trimmed_display_name VARCHAR(256);
    resolved_is_default BOOLEAN;
    resolved_page_size INTEGER;
    resolved_default_media_domain_id BIGINT;
    normalized_domain_key VARCHAR(128);
    new_profile_id BIGINT;
    new_profile_public_id UUID;
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

    IF user_public_id_input IS NULL THEN
        IF actor_role NOT IN ('owner', 'admin') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_unauthorized';
        END IF;
        target_user_id := NULL;
    ELSE
        SELECT user_id
        INTO target_user_id
        FROM app_user
        WHERE user_public_id = user_public_id_input;

        IF target_user_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'user_not_found';
        END IF;

        IF actor_role NOT IN ('owner', 'admin') AND target_user_id <> actor_user_id THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_unauthorized';
        END IF;
    END IF;

    resolved_page_size := COALESCE(page_size_input, 50);
    IF resolved_page_size < 10 THEN
        resolved_page_size := 10;
    ELSIF resolved_page_size > 200 THEN
        resolved_page_size := 200;
    END IF;

    IF default_media_domain_key_input IS NOT NULL THEN
        normalized_domain_key := lower(trim(default_media_domain_key_input));

        IF normalized_domain_key = '' OR normalized_domain_key <> default_media_domain_key_input THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'media_domain_key_invalid';
        END IF;

        SELECT media_domain_id
        INTO resolved_default_media_domain_id
        FROM media_domain
        WHERE media_domain_key::TEXT = normalized_domain_key;

        IF resolved_default_media_domain_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'media_domain_not_found';
        END IF;
    ELSE
        resolved_default_media_domain_id := NULL;
    END IF;

    resolved_is_default := COALESCE(is_default_input, FALSE);

    IF resolved_is_default THEN
        UPDATE search_profile
        SET is_default = FALSE,
            updated_by_user_id = actor_user_id,
            updated_at = now()
        WHERE deleted_at IS NULL
          AND (
              (target_user_id IS NULL AND user_id IS NULL)
              OR user_id = target_user_id
          );
    END IF;

    new_profile_public_id := gen_random_uuid();

    INSERT INTO search_profile (
        search_profile_public_id,
        user_id,
        display_name,
        is_default,
        page_size,
        default_media_domain_id,
        created_by_user_id,
        updated_by_user_id
    )
    VALUES (
        new_profile_public_id,
        target_user_id,
        trimmed_display_name,
        resolved_is_default,
        resolved_page_size,
        resolved_default_media_domain_id,
        actor_user_id,
        actor_user_id
    )
    RETURNING search_profile_id INTO new_profile_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'search_profile',
        new_profile_id,
        new_profile_public_id,
        'create',
        actor_user_id,
        'search_profile_create'
    );

    RETURN new_profile_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_create(
    actor_user_public_id UUID,
    display_name_input VARCHAR,
    is_default_input BOOLEAN,
    page_size_input INTEGER,
    default_media_domain_key_input VARCHAR,
    user_public_id_input UUID
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN search_profile_create_v1(
        actor_user_public_id,
        display_name_input,
        is_default_input,
        page_size_input,
        default_media_domain_key_input,
        user_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_update_v1(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    display_name_input VARCHAR,
    page_size_input INTEGER
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to update search profile';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    trimmed_display_name VARCHAR(256);
    resolved_page_size INTEGER;
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

    SELECT search_profile_id, user_id, deleted_at
    INTO profile_id, profile_user_id, profile_deleted_at
    FROM search_profile
    WHERE search_profile_public_id = search_profile_public_id_input;

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

    IF page_size_input IS NOT NULL THEN
        resolved_page_size := page_size_input;
        IF resolved_page_size < 10 THEN
            resolved_page_size := 10;
        ELSIF resolved_page_size > 200 THEN
            resolved_page_size := 200;
        END IF;
    ELSE
        resolved_page_size := NULL;
    END IF;

    UPDATE search_profile
    SET display_name = trimmed_display_name,
        page_size = COALESCE(resolved_page_size, page_size),
        updated_by_user_id = actor_user_id,
        updated_at = now()
    WHERE search_profile_id = profile_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'search_profile',
        profile_id,
        search_profile_public_id_input,
        'update',
        actor_user_id,
        'search_profile_update'
    );

    RETURN search_profile_public_id_input;
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_update(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    display_name_input VARCHAR,
    page_size_input INTEGER
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN search_profile_update_v1(
        actor_user_public_id,
        search_profile_public_id_input,
        display_name_input,
        page_size_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_set_default_v1(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    page_size_input INTEGER
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to set default search profile';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    resolved_page_size INTEGER;
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

    SELECT search_profile_id, user_id, deleted_at
    INTO profile_id, profile_user_id, profile_deleted_at
    FROM search_profile
    WHERE search_profile_public_id = search_profile_public_id_input;

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

    IF page_size_input IS NOT NULL THEN
        resolved_page_size := page_size_input;
        IF resolved_page_size < 10 THEN
            resolved_page_size := 10;
        ELSIF resolved_page_size > 200 THEN
            resolved_page_size := 200;
        END IF;
    ELSE
        resolved_page_size := NULL;
    END IF;

    UPDATE search_profile
    SET is_default = FALSE,
        updated_by_user_id = actor_user_id,
        updated_at = now()
    WHERE deleted_at IS NULL
      AND (
          (profile_user_id IS NULL AND user_id IS NULL)
          OR user_id = profile_user_id
      );

    UPDATE search_profile
    SET is_default = TRUE,
        page_size = COALESCE(resolved_page_size, page_size),
        updated_by_user_id = actor_user_id,
        updated_at = now()
    WHERE search_profile_id = profile_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'search_profile',
        profile_id,
        search_profile_public_id_input,
        'update',
        actor_user_id,
        'search_profile_set_default'
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_set_default(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    page_size_input INTEGER
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_profile_set_default_v1(
        actor_user_public_id,
        search_profile_public_id_input,
        page_size_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_set_default_domain_v1(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    default_media_domain_key_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to set default media domain';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    resolved_domain_id BIGINT;
    normalized_domain_key VARCHAR(128);
    allowlist_count INTEGER;
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

    SELECT search_profile_id, user_id, deleted_at
    INTO profile_id, profile_user_id, profile_deleted_at
    FROM search_profile
    WHERE search_profile_public_id = search_profile_public_id_input;

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

    IF default_media_domain_key_input IS NULL THEN
        resolved_domain_id := NULL;
    ELSE
        normalized_domain_key := lower(trim(default_media_domain_key_input));

        IF normalized_domain_key = '' OR normalized_domain_key <> default_media_domain_key_input THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'media_domain_key_invalid';
        END IF;

        SELECT media_domain_id
        INTO resolved_domain_id
        FROM media_domain
        WHERE media_domain_key::TEXT = normalized_domain_key;

        IF resolved_domain_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'media_domain_not_found';
        END IF;
    END IF;

    SELECT count(*)
    INTO allowlist_count
    FROM search_profile_media_domain
    WHERE search_profile_id = profile_id;

    IF allowlist_count > 0 AND resolved_domain_id IS NOT NULL THEN
        IF NOT EXISTS (
            SELECT 1
            FROM search_profile_media_domain
            WHERE search_profile_id = profile_id
              AND media_domain_id = resolved_domain_id
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'default_not_in_allowlist';
        END IF;
    END IF;

    UPDATE search_profile
    SET default_media_domain_id = resolved_domain_id,
        updated_by_user_id = actor_user_id,
        updated_at = now()
    WHERE search_profile_id = profile_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'search_profile',
        profile_id,
        search_profile_public_id_input,
        'update',
        actor_user_id,
        'search_profile_set_default_domain'
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_set_default_domain(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    default_media_domain_key_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_profile_set_default_domain_v1(
        actor_user_public_id,
        search_profile_public_id_input,
        default_media_domain_key_input
    );
END;
$$;

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
    default_media_domain_id BIGINT;
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

    SELECT search_profile_id, user_id, deleted_at, default_media_domain_id
    INTO profile_id, profile_user_id, profile_deleted_at, default_media_domain_id
    FROM search_profile
    WHERE search_profile_public_id = search_profile_public_id_input;

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
                DETAIL = 'media_domain_not_found';
        END IF;

        IF default_media_domain_id IS NOT NULL THEN
            IF NOT EXISTS (
                SELECT 1
                FROM media_domain
                WHERE media_domain_id = default_media_domain_id
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
        actor_user_public_id,
        search_profile_public_id_input,
        media_domain_keys_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_add_policy_set_v1(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    policy_set_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to add policy set';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    policy_set_id_value BIGINT;
    policy_scope_value policy_scope;
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

    IF search_profile_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_profile_missing';
    END IF;

    IF policy_set_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_set_missing';
    END IF;

    SELECT search_profile_id, user_id, deleted_at
    INTO profile_id, profile_user_id, profile_deleted_at
    FROM search_profile
    WHERE search_profile_public_id = search_profile_public_id_input;

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

    SELECT policy_set_id, scope, deleted_at
    INTO policy_set_id_value, policy_scope_value, policy_deleted_at
    FROM policy_set
    WHERE policy_set_public_id = policy_set_public_id_input;

    IF policy_set_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_set_not_found';
    END IF;

    IF policy_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_set_deleted';
    END IF;

    IF policy_scope_value <> 'profile' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_set_invalid_scope';
    END IF;

    INSERT INTO search_profile_policy_set (
        search_profile_id,
        policy_set_id
    )
    VALUES (
        profile_id,
        policy_set_id_value
    )
    ON CONFLICT (search_profile_id, policy_set_id) DO NOTHING;

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
        'search_profile_policy_set_add'
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_add_policy_set(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    policy_set_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_profile_add_policy_set_v1(
        actor_user_public_id,
        search_profile_public_id_input,
        policy_set_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_remove_policy_set_v1(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    policy_set_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to remove policy set';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    policy_set_id_value BIGINT;
    policy_scope_value policy_scope;
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

    IF search_profile_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_profile_missing';
    END IF;

    IF policy_set_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_set_missing';
    END IF;

    SELECT search_profile_id, user_id, deleted_at
    INTO profile_id, profile_user_id, profile_deleted_at
    FROM search_profile
    WHERE search_profile_public_id = search_profile_public_id_input;

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

    SELECT policy_set_id, scope, deleted_at
    INTO policy_set_id_value, policy_scope_value, policy_deleted_at
    FROM policy_set
    WHERE policy_set_public_id = policy_set_public_id_input;

    IF policy_set_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_set_not_found';
    END IF;

    IF policy_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_set_deleted';
    END IF;

    IF policy_scope_value <> 'profile' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_set_invalid_scope';
    END IF;

    DELETE FROM search_profile_policy_set
    WHERE search_profile_id = profile_id
      AND policy_set_id = policy_set_id_value;

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
        'search_profile_policy_set_remove'
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_remove_policy_set(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    policy_set_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_profile_remove_policy_set_v1(
        actor_user_public_id,
        search_profile_public_id_input,
        policy_set_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_indexer_allow_v1(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    indexer_instance_public_ids_input UUID[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to set indexer allowlist';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    input_count INTEGER;
    resolved_count INTEGER;
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

    SELECT search_profile_id, user_id, deleted_at
    INTO profile_id, profile_user_id, profile_deleted_at
    FROM search_profile
    WHERE search_profile_public_id = search_profile_public_id_input;

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

    IF indexer_instance_public_ids_input IS NULL THEN
        DELETE FROM search_profile_indexer_allow
        WHERE search_profile_id = profile_id;
    ELSE
        IF EXISTS (
            SELECT 1
            FROM unnest(indexer_instance_public_ids_input) AS value
            WHERE value IS NULL
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'indexer_id_invalid';
        END IF;

        SELECT count(DISTINCT value)
        INTO input_count
        FROM unnest(indexer_instance_public_ids_input) AS value;

        IF input_count = 0 THEN
            DELETE FROM search_profile_indexer_allow
            WHERE search_profile_id = profile_id;
        ELSE
            SELECT count(*)
            INTO resolved_count
            FROM indexer_instance
            WHERE indexer_instance_public_id = ANY(indexer_instance_public_ids_input)
              AND deleted_at IS NULL;

            IF resolved_count <> input_count THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'indexer_not_found';
            END IF;

            IF EXISTS (
                SELECT 1
                FROM search_profile_indexer_block
                WHERE search_profile_id = profile_id
                  AND indexer_instance_id IN (
                      SELECT indexer_instance_id
                      FROM indexer_instance
                      WHERE indexer_instance_public_id = ANY(indexer_instance_public_ids_input)
                        AND deleted_at IS NULL
                  )
            ) THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'indexer_block_conflict';
            END IF;

            DELETE FROM search_profile_indexer_allow
            WHERE search_profile_id = profile_id;

            INSERT INTO search_profile_indexer_allow (
                search_profile_id,
                indexer_instance_id
            )
            SELECT
                profile_id,
                indexer_instance_id
            FROM indexer_instance
            WHERE indexer_instance_public_id = ANY(indexer_instance_public_ids_input)
              AND deleted_at IS NULL;
        END IF;
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
        'search_profile_indexer_allow'
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_indexer_allow(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    indexer_instance_public_ids_input UUID[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_profile_indexer_allow_v1(
        actor_user_public_id,
        search_profile_public_id_input,
        indexer_instance_public_ids_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_indexer_block_v1(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    indexer_instance_public_ids_input UUID[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to set indexer blocklist';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    input_count INTEGER;
    resolved_count INTEGER;
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

    SELECT search_profile_id, user_id, deleted_at
    INTO profile_id, profile_user_id, profile_deleted_at
    FROM search_profile
    WHERE search_profile_public_id = search_profile_public_id_input;

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

    IF indexer_instance_public_ids_input IS NULL THEN
        DELETE FROM search_profile_indexer_block
        WHERE search_profile_id = profile_id;
    ELSE
        IF EXISTS (
            SELECT 1
            FROM unnest(indexer_instance_public_ids_input) AS value
            WHERE value IS NULL
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'indexer_id_invalid';
        END IF;

        SELECT count(DISTINCT value)
        INTO input_count
        FROM unnest(indexer_instance_public_ids_input) AS value;

        IF input_count = 0 THEN
            DELETE FROM search_profile_indexer_block
            WHERE search_profile_id = profile_id;
        ELSE
            SELECT count(*)
            INTO resolved_count
            FROM indexer_instance
            WHERE indexer_instance_public_id = ANY(indexer_instance_public_ids_input)
              AND deleted_at IS NULL;

            IF resolved_count <> input_count THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'indexer_not_found';
            END IF;

            IF EXISTS (
                SELECT 1
                FROM search_profile_indexer_allow
                WHERE search_profile_id = profile_id
                  AND indexer_instance_id IN (
                      SELECT indexer_instance_id
                      FROM indexer_instance
                      WHERE indexer_instance_public_id = ANY(indexer_instance_public_ids_input)
                        AND deleted_at IS NULL
                  )
            ) THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'indexer_allow_conflict';
            END IF;

            DELETE FROM search_profile_indexer_block
            WHERE search_profile_id = profile_id;

            INSERT INTO search_profile_indexer_block (
                search_profile_id,
                indexer_instance_id
            )
            SELECT
                profile_id,
                indexer_instance_id
            FROM indexer_instance
            WHERE indexer_instance_public_id = ANY(indexer_instance_public_ids_input)
              AND deleted_at IS NULL;
        END IF;
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
        'search_profile_indexer_block'
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_indexer_block(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    indexer_instance_public_ids_input UUID[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_profile_indexer_block_v1(
        actor_user_public_id,
        search_profile_public_id_input,
        indexer_instance_public_ids_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_tag_allow_v1(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    tag_public_ids_input UUID[],
    tag_keys_input TEXT[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to set tag allowlist';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    resolved_tag_ids UUID[];
    resolved_tag_ids_from_keys UUID[];
    normalized_keys TEXT[];
    public_count INTEGER;
    public_resolved INTEGER;
    key_count INTEGER;
    key_resolved INTEGER;
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

    SELECT search_profile_id, user_id, deleted_at
    INTO profile_id, profile_user_id, profile_deleted_at
    FROM search_profile
    WHERE search_profile_public_id = search_profile_public_id_input;

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

    resolved_tag_ids := ARRAY[]::UUID[];
    resolved_tag_ids_from_keys := ARRAY[]::UUID[];

    IF tag_public_ids_input IS NOT NULL THEN
        SELECT array_agg(DISTINCT tag_public_id), count(DISTINCT tag_public_id)
        INTO resolved_tag_ids, public_resolved
        FROM tag
        WHERE tag_public_id = ANY(tag_public_ids_input)
          AND deleted_at IS NULL;

        SELECT count(DISTINCT value)
        INTO public_count
        FROM unnest(tag_public_ids_input) AS value;

        IF public_resolved IS NULL THEN
            public_resolved := 0;
        END IF;

        IF public_resolved <> public_count THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_not_found';
        END IF;
    END IF;

    IF tag_keys_input IS NOT NULL THEN
        SELECT array_agg(DISTINCT lower(trim(value)))
        INTO normalized_keys
        FROM unnest(tag_keys_input) AS value;

        IF EXISTS (
            SELECT 1
            FROM unnest(tag_keys_input) AS value
            WHERE trim(value) = ''
               OR trim(value) <> lower(trim(value))
               OR char_length(trim(value)) > 128
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_key_invalid';
        END IF;

        SELECT count(*)
        INTO key_count
        FROM unnest(normalized_keys) AS value
        WHERE value IS NOT NULL AND value <> '';

        IF key_count = 0 THEN
            normalized_keys := ARRAY[]::TEXT[];
        END IF;

        SELECT array_agg(DISTINCT tag_public_id), count(DISTINCT tag_public_id)
        INTO resolved_tag_ids_from_keys, key_resolved
        FROM tag
        WHERE tag_key = ANY(normalized_keys)
          AND deleted_at IS NULL;

        IF key_resolved IS NULL THEN
            key_resolved := 0;
        END IF;

        IF key_resolved <> key_count THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_not_found';
        END IF;
    END IF;

    IF tag_public_ids_input IS NOT NULL AND tag_keys_input IS NOT NULL THEN
        IF EXISTS (
            SELECT value
            FROM unnest(resolved_tag_ids) AS value
            EXCEPT
            SELECT value
            FROM unnest(resolved_tag_ids_from_keys) AS value
        ) OR EXISTS (
            SELECT value
            FROM unnest(resolved_tag_ids_from_keys) AS value
            EXCEPT
            SELECT value
            FROM unnest(resolved_tag_ids) AS value
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_tag_reference';
        END IF;
    END IF;

    IF resolved_tag_ids IS NULL OR array_length(resolved_tag_ids, 1) IS NULL THEN
        resolved_tag_ids := resolved_tag_ids_from_keys;
    END IF;

    IF resolved_tag_ids IS NULL THEN
        resolved_tag_ids := ARRAY[]::UUID[];
    END IF;

    IF array_length(resolved_tag_ids, 1) IS NOT NULL THEN
        IF EXISTS (
            SELECT 1
            FROM search_profile_tag_block
            WHERE search_profile_id = profile_id
              AND tag_id IN (
                  SELECT tag_id
                  FROM tag
                  WHERE tag_public_id = ANY(resolved_tag_ids)
                    AND deleted_at IS NULL
              )
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_block_conflict';
        END IF;
    END IF;

    DELETE FROM search_profile_tag_allow
    WHERE search_profile_id = profile_id;

    INSERT INTO search_profile_tag_allow (
        search_profile_id,
        tag_id
    )
    SELECT
        profile_id,
        tag_id
    FROM tag
    WHERE tag_public_id = ANY(resolved_tag_ids)
      AND deleted_at IS NULL;

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
        'search_profile_tag_allow'
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_tag_allow(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    tag_public_ids_input UUID[],
    tag_keys_input TEXT[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_profile_tag_allow_v1(
        actor_user_public_id,
        search_profile_public_id_input,
        tag_public_ids_input,
        tag_keys_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_tag_block_v1(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    tag_public_ids_input UUID[],
    tag_keys_input TEXT[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to set tag blocklist';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    resolved_tag_ids UUID[];
    resolved_tag_ids_from_keys UUID[];
    normalized_keys TEXT[];
    public_count INTEGER;
    public_resolved INTEGER;
    key_count INTEGER;
    key_resolved INTEGER;
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

    SELECT search_profile_id, user_id, deleted_at
    INTO profile_id, profile_user_id, profile_deleted_at
    FROM search_profile
    WHERE search_profile_public_id = search_profile_public_id_input;

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

    resolved_tag_ids := ARRAY[]::UUID[];
    resolved_tag_ids_from_keys := ARRAY[]::UUID[];

    IF tag_public_ids_input IS NOT NULL THEN
        SELECT array_agg(DISTINCT tag_public_id), count(DISTINCT tag_public_id)
        INTO resolved_tag_ids, public_resolved
        FROM tag
        WHERE tag_public_id = ANY(tag_public_ids_input)
          AND deleted_at IS NULL;

        SELECT count(DISTINCT value)
        INTO public_count
        FROM unnest(tag_public_ids_input) AS value;

        IF public_resolved IS NULL THEN
            public_resolved := 0;
        END IF;

        IF public_resolved <> public_count THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_not_found';
        END IF;
    END IF;

    IF tag_keys_input IS NOT NULL THEN
        SELECT array_agg(DISTINCT lower(trim(value)))
        INTO normalized_keys
        FROM unnest(tag_keys_input) AS value;

        IF EXISTS (
            SELECT 1
            FROM unnest(tag_keys_input) AS value
            WHERE trim(value) = ''
               OR trim(value) <> lower(trim(value))
               OR char_length(trim(value)) > 128
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_key_invalid';
        END IF;

        SELECT count(*)
        INTO key_count
        FROM unnest(normalized_keys) AS value
        WHERE value IS NOT NULL AND value <> '';

        IF key_count = 0 THEN
            normalized_keys := ARRAY[]::TEXT[];
        END IF;

        SELECT array_agg(DISTINCT tag_public_id), count(DISTINCT tag_public_id)
        INTO resolved_tag_ids_from_keys, key_resolved
        FROM tag
        WHERE tag_key = ANY(normalized_keys)
          AND deleted_at IS NULL;

        IF key_resolved IS NULL THEN
            key_resolved := 0;
        END IF;

        IF key_resolved <> key_count THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_not_found';
        END IF;
    END IF;

    IF tag_public_ids_input IS NOT NULL AND tag_keys_input IS NOT NULL THEN
        IF EXISTS (
            SELECT value
            FROM unnest(resolved_tag_ids) AS value
            EXCEPT
            SELECT value
            FROM unnest(resolved_tag_ids_from_keys) AS value
        ) OR EXISTS (
            SELECT value
            FROM unnest(resolved_tag_ids_from_keys) AS value
            EXCEPT
            SELECT value
            FROM unnest(resolved_tag_ids) AS value
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_tag_reference';
        END IF;
    END IF;

    IF resolved_tag_ids IS NULL OR array_length(resolved_tag_ids, 1) IS NULL THEN
        resolved_tag_ids := resolved_tag_ids_from_keys;
    END IF;

    IF resolved_tag_ids IS NULL THEN
        resolved_tag_ids := ARRAY[]::UUID[];
    END IF;

    IF array_length(resolved_tag_ids, 1) IS NOT NULL THEN
        IF EXISTS (
            SELECT 1
            FROM search_profile_tag_allow
            WHERE search_profile_id = profile_id
              AND tag_id IN (
                  SELECT tag_id
                  FROM tag
                  WHERE tag_public_id = ANY(resolved_tag_ids)
                    AND deleted_at IS NULL
              )
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_allow_conflict';
        END IF;
    END IF;

    DELETE FROM search_profile_tag_block
    WHERE search_profile_id = profile_id;

    INSERT INTO search_profile_tag_block (
        search_profile_id,
        tag_id
    )
    SELECT
        profile_id,
        tag_id
    FROM tag
    WHERE tag_public_id = ANY(resolved_tag_ids)
      AND deleted_at IS NULL;

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
        'search_profile_tag_block'
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_tag_block(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    tag_public_ids_input UUID[],
    tag_keys_input TEXT[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_profile_tag_block_v1(
        actor_user_public_id,
        search_profile_public_id_input,
        tag_public_ids_input,
        tag_keys_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_tag_prefer_v1(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    tag_public_ids_input UUID[],
    tag_keys_input TEXT[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to set tag preferences';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    resolved_tag_ids UUID[];
    resolved_tag_ids_from_keys UUID[];
    normalized_keys TEXT[];
    public_count INTEGER;
    public_resolved INTEGER;
    key_count INTEGER;
    key_resolved INTEGER;
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

    SELECT search_profile_id, user_id, deleted_at
    INTO profile_id, profile_user_id, profile_deleted_at
    FROM search_profile
    WHERE search_profile_public_id = search_profile_public_id_input;

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

    resolved_tag_ids := ARRAY[]::UUID[];
    resolved_tag_ids_from_keys := ARRAY[]::UUID[];

    IF tag_public_ids_input IS NOT NULL THEN
        SELECT array_agg(DISTINCT tag_public_id), count(DISTINCT tag_public_id)
        INTO resolved_tag_ids, public_resolved
        FROM tag
        WHERE tag_public_id = ANY(tag_public_ids_input)
          AND deleted_at IS NULL;

        SELECT count(DISTINCT value)
        INTO public_count
        FROM unnest(tag_public_ids_input) AS value;

        IF public_resolved IS NULL THEN
            public_resolved := 0;
        END IF;

        IF public_resolved <> public_count THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_not_found';
        END IF;
    END IF;

    IF tag_keys_input IS NOT NULL THEN
        SELECT array_agg(DISTINCT lower(trim(value)))
        INTO normalized_keys
        FROM unnest(tag_keys_input) AS value;

        IF EXISTS (
            SELECT 1
            FROM unnest(tag_keys_input) AS value
            WHERE trim(value) = ''
               OR trim(value) <> lower(trim(value))
               OR char_length(trim(value)) > 128
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_key_invalid';
        END IF;

        SELECT count(*)
        INTO key_count
        FROM unnest(normalized_keys) AS value
        WHERE value IS NOT NULL AND value <> '';

        IF key_count = 0 THEN
            normalized_keys := ARRAY[]::TEXT[];
        END IF;

        SELECT array_agg(DISTINCT tag_public_id), count(DISTINCT tag_public_id)
        INTO resolved_tag_ids_from_keys, key_resolved
        FROM tag
        WHERE tag_key = ANY(normalized_keys)
          AND deleted_at IS NULL;

        IF key_resolved IS NULL THEN
            key_resolved := 0;
        END IF;

        IF key_resolved <> key_count THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_not_found';
        END IF;
    END IF;

    IF tag_public_ids_input IS NOT NULL AND tag_keys_input IS NOT NULL THEN
        IF EXISTS (
            SELECT value
            FROM unnest(resolved_tag_ids) AS value
            EXCEPT
            SELECT value
            FROM unnest(resolved_tag_ids_from_keys) AS value
        ) OR EXISTS (
            SELECT value
            FROM unnest(resolved_tag_ids_from_keys) AS value
            EXCEPT
            SELECT value
            FROM unnest(resolved_tag_ids) AS value
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_tag_reference';
        END IF;
    END IF;

    IF resolved_tag_ids IS NULL OR array_length(resolved_tag_ids, 1) IS NULL THEN
        resolved_tag_ids := resolved_tag_ids_from_keys;
    END IF;

    IF resolved_tag_ids IS NULL THEN
        resolved_tag_ids := ARRAY[]::UUID[];
    END IF;

    IF array_length(resolved_tag_ids, 1) IS NOT NULL THEN
        IF EXISTS (
            SELECT 1
            FROM search_profile_tag_block
            WHERE search_profile_id = profile_id
              AND tag_id IN (
                  SELECT tag_id
                  FROM tag
                  WHERE tag_public_id = ANY(resolved_tag_ids)
                    AND deleted_at IS NULL
              )
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_block_conflict';
        END IF;
    END IF;

    DELETE FROM search_profile_tag_prefer
    WHERE search_profile_id = profile_id;

    INSERT INTO search_profile_tag_prefer (
        search_profile_id,
        tag_id
    )
    SELECT
        profile_id,
        tag_id
    FROM tag
    WHERE tag_public_id = ANY(resolved_tag_ids)
      AND deleted_at IS NULL;

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
        'search_profile_tag_prefer'
    );
END;
$$;

CREATE OR REPLACE FUNCTION search_profile_tag_prefer(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    tag_public_ids_input UUID[],
    tag_keys_input TEXT[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM search_profile_tag_prefer_v1(
        actor_user_public_id,
        search_profile_public_id_input,
        tag_public_ids_input,
        tag_keys_input
    );
END;
$$;
