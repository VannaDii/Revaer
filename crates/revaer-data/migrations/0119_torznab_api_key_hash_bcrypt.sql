-- Use a pgcrypto-supported algorithm for torznab API-key hashing.

CREATE OR REPLACE FUNCTION torznab_instance_create_v1(
    actor_user_public_id UUID,
    search_profile_public_id_input UUID,
    display_name_input VARCHAR
)
RETURNS TABLE(
    torznab_instance_public_id UUID,
    api_key_plaintext VARCHAR
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to create torznab instance';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    instance_id BIGINT;
    instance_public_id UUID;
    trimmed_display_name VARCHAR(256);
    raw_key TEXT;
    api_key_hash_value TEXT;
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

    IF EXISTS (
        SELECT 1
        FROM torznab_instance
        WHERE display_name = trimmed_display_name
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'display_name_already_exists';
    END IF;

    raw_key := encode(gen_random_bytes(32), 'base64');
    api_key_plaintext := regexp_replace(translate(raw_key, '+/', '-_'), '=', '', 'g');
    api_key_hash_value := crypt(api_key_plaintext, gen_salt('bf', 12));

    IF api_key_hash_value IS NULL OR api_key_hash_value = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'api_key_hash_failed';
    END IF;

    instance_public_id := gen_random_uuid();

    INSERT INTO torznab_instance (
        search_profile_id,
        torznab_instance_public_id,
        display_name,
        api_key_hash,
        is_enabled
    )
    VALUES (
        profile_id,
        instance_public_id,
        trimmed_display_name,
        api_key_hash_value,
        TRUE
    )
    RETURNING torznab_instance_id INTO instance_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'torznab_instance',
        instance_id,
        instance_public_id,
        'create',
        actor_user_id,
        'torznab_instance_create'
    );

    torznab_instance_public_id := instance_public_id;
    RETURN NEXT;
END;
$$;

CREATE OR REPLACE FUNCTION torznab_instance_rotate_key_v1(
    actor_user_public_id UUID,
    torznab_instance_public_id_input UUID
)
RETURNS VARCHAR
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to rotate torznab api key';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    instance_deleted_at TIMESTAMPTZ;
    raw_key TEXT;
    api_key_plaintext_value TEXT;
    api_key_hash_value TEXT;
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

    IF torznab_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'torznab_instance_missing';
    END IF;

    SELECT torznab_instance_id, search_profile_id, deleted_at
    INTO instance_id, profile_id, instance_deleted_at
    FROM torznab_instance
    WHERE torznab_instance_public_id = torznab_instance_public_id_input;

    IF instance_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'torznab_instance_not_found';
    END IF;

    IF instance_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'torznab_instance_deleted';
    END IF;

    SELECT user_id, deleted_at
    INTO profile_user_id, profile_deleted_at
    FROM search_profile
    WHERE search_profile_id = profile_id;

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

    raw_key := encode(gen_random_bytes(32), 'base64');
    api_key_plaintext_value := regexp_replace(translate(raw_key, '+/', '-_'), '=', '', 'g');
    api_key_hash_value := crypt(api_key_plaintext_value, gen_salt('bf', 12));

    IF api_key_hash_value IS NULL OR api_key_hash_value = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'api_key_hash_failed';
    END IF;

    UPDATE torznab_instance
    SET api_key_hash = api_key_hash_value,
        updated_at = now()
    WHERE torznab_instance_id = instance_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'torznab_instance',
        instance_id,
        torznab_instance_public_id_input,
        'update',
        actor_user_id,
        'torznab_instance_rotate_key'
    );

    RETURN api_key_plaintext_value;
END;
$$;
