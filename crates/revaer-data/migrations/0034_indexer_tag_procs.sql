-- Stored procedures for tag management.

CREATE OR REPLACE FUNCTION tag_create_v1(
    actor_user_public_id UUID,
    tag_key_input VARCHAR,
    display_name_input VARCHAR
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to create tag';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    new_tag_id BIGINT;
    new_tag_public_id UUID;
    trimmed_tag_key VARCHAR(128);
    trimmed_display_name VARCHAR(256);
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

    IF tag_key_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tag_key_missing';
    END IF;

    trimmed_tag_key := trim(tag_key_input);

    IF trimmed_tag_key = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tag_key_empty';
    END IF;

    IF trimmed_tag_key <> lower(trimmed_tag_key) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tag_key_not_lowercase';
    END IF;

    IF char_length(trimmed_tag_key) > 128 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tag_key_too_long';
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
        FROM tag
        WHERE tag_key = trimmed_tag_key
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tag_key_already_exists';
    END IF;

    new_tag_public_id := gen_random_uuid();

    INSERT INTO tag (
        tag_public_id,
        tag_key,
        display_name,
        created_by_user_id,
        updated_by_user_id
    )
    VALUES (
        new_tag_public_id,
        trimmed_tag_key,
        trimmed_display_name,
        actor_user_id,
        actor_user_id
    )
    RETURNING tag_id INTO new_tag_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'tag',
        new_tag_id,
        new_tag_public_id,
        'create',
        actor_user_id,
        'tag_create'
    );

    RETURN new_tag_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION tag_create(
    actor_user_public_id UUID,
    tag_key_input VARCHAR,
    display_name_input VARCHAR
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN tag_create_v1(actor_user_public_id, tag_key_input, display_name_input);
END;
$$;

CREATE OR REPLACE FUNCTION tag_update_v1(
    actor_user_public_id UUID,
    tag_public_id_input UUID,
    tag_key_input VARCHAR,
    display_name_input VARCHAR
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to update tag';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    resolved_tag_id BIGINT;
    resolved_tag_public_id UUID;
    resolved_tag_key VARCHAR(128);
    resolved_deleted_at TIMESTAMPTZ;
    trimmed_tag_key VARCHAR(128);
    trimmed_display_name VARCHAR(256);
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

    IF tag_public_id_input IS NULL AND tag_key_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tag_reference_missing';
    END IF;

    IF tag_key_input IS NOT NULL THEN
        trimmed_tag_key := trim(tag_key_input);

        IF trimmed_tag_key = '' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_key_empty';
        END IF;

        IF trimmed_tag_key <> lower(trimmed_tag_key) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_key_not_lowercase';
        END IF;

        IF char_length(trimmed_tag_key) > 128 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_key_too_long';
        END IF;
    END IF;

    IF tag_public_id_input IS NOT NULL THEN
        SELECT tag_id, tag_public_id, tag_key, deleted_at
        INTO resolved_tag_id, resolved_tag_public_id, resolved_tag_key, resolved_deleted_at
        FROM tag
        WHERE tag_public_id = tag_public_id_input;

        IF resolved_tag_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_not_found';
        END IF;

        IF trimmed_tag_key IS NOT NULL AND trimmed_tag_key <> resolved_tag_key THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_tag_reference';
        END IF;
    ELSE
        SELECT tag_id, tag_public_id, tag_key, deleted_at
        INTO resolved_tag_id, resolved_tag_public_id, resolved_tag_key, resolved_deleted_at
        FROM tag
        WHERE tag_key = trimmed_tag_key;

        IF resolved_tag_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_not_found';
        END IF;
    END IF;

    IF resolved_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tag_deleted';
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

    UPDATE tag
    SET display_name = trimmed_display_name,
        updated_by_user_id = actor_user_id,
        updated_at = now()
    WHERE tag_id = resolved_tag_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'tag',
        resolved_tag_id,
        resolved_tag_public_id,
        'update',
        actor_user_id,
        'tag_update'
    );

    RETURN resolved_tag_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION tag_update(
    actor_user_public_id UUID,
    tag_public_id_input UUID,
    tag_key_input VARCHAR,
    display_name_input VARCHAR
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN tag_update_v1(
        actor_user_public_id,
        tag_public_id_input,
        tag_key_input,
        display_name_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION tag_soft_delete_v1(
    actor_user_public_id UUID,
    tag_public_id_input UUID,
    tag_key_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to delete tag';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    resolved_tag_id BIGINT;
    resolved_tag_public_id UUID;
    resolved_tag_key VARCHAR(128);
    trimmed_tag_key VARCHAR(128);
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

    IF tag_public_id_input IS NULL AND tag_key_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tag_reference_missing';
    END IF;

    IF tag_key_input IS NOT NULL THEN
        trimmed_tag_key := trim(tag_key_input);

        IF trimmed_tag_key = '' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_key_empty';
        END IF;

        IF trimmed_tag_key <> lower(trimmed_tag_key) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_key_not_lowercase';
        END IF;

        IF char_length(trimmed_tag_key) > 128 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_key_too_long';
        END IF;
    END IF;

    IF tag_public_id_input IS NOT NULL THEN
        SELECT tag_id, tag_public_id, tag_key
        INTO resolved_tag_id, resolved_tag_public_id, resolved_tag_key
        FROM tag
        WHERE tag_public_id = tag_public_id_input;

        IF resolved_tag_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_not_found';
        END IF;

        IF trimmed_tag_key IS NOT NULL AND trimmed_tag_key <> resolved_tag_key THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'invalid_tag_reference';
        END IF;
    ELSE
        SELECT tag_id, tag_public_id, tag_key
        INTO resolved_tag_id, resolved_tag_public_id, resolved_tag_key
        FROM tag
        WHERE tag_key = trimmed_tag_key;

        IF resolved_tag_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'tag_not_found';
        END IF;
    END IF;

    UPDATE tag
    SET deleted_at = COALESCE(deleted_at, now()),
        updated_by_user_id = actor_user_id,
        updated_at = now()
    WHERE tag_id = resolved_tag_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'tag',
        resolved_tag_id,
        resolved_tag_public_id,
        'soft_delete',
        actor_user_id,
        'tag_soft_delete'
    );
END;
$$;

CREATE OR REPLACE FUNCTION tag_soft_delete(
    actor_user_public_id UUID,
    tag_public_id_input UUID,
    tag_key_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM tag_soft_delete_v1(actor_user_public_id, tag_public_id_input, tag_key_input);
END;
$$;
