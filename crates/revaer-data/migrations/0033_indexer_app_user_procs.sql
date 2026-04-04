-- Stored procedures for app_user management.

CREATE OR REPLACE FUNCTION app_user_create_v1(
    email_input VARCHAR,
    display_name_input VARCHAR
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to create app user';
    errcode CONSTANT text := 'P0001';
    normalized_email VARCHAR(320);
    new_public_id UUID;
    trimmed_email VARCHAR(320);
    trimmed_display_name VARCHAR(256);
BEGIN
    IF email_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'email_missing';
    END IF;

    trimmed_email := trim(email_input);
    normalized_email := lower(trimmed_email);

    IF normalized_email = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'email_empty';
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

    IF EXISTS (
        SELECT 1
        FROM app_user
        WHERE email_normalized = normalized_email
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'email_already_registered';
    END IF;

    new_public_id := gen_random_uuid();

    INSERT INTO app_user (
        user_public_id,
        email,
        email_normalized,
        is_email_verified,
        display_name,
        role
    )
    VALUES (
        new_public_id,
        trimmed_email,
        normalized_email,
        FALSE,
        trimmed_display_name,
        'user'
    )
    RETURNING user_public_id INTO new_public_id;

    RETURN new_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION app_user_create(
    email_input VARCHAR,
    display_name_input VARCHAR
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN app_user_create_v1(email_input, display_name_input);
END;
$$;

CREATE OR REPLACE FUNCTION app_user_update_v1(
    user_public_id_input UUID,
    display_name_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to update app user';
    errcode CONSTANT text := 'P0001';
    trimmed_display_name VARCHAR(256);
BEGIN
    IF user_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'user_missing';
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

    UPDATE app_user
    SET display_name = trimmed_display_name
    WHERE user_public_id = user_public_id_input;

    IF NOT FOUND THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'user_not_found';
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION app_user_update(
    user_public_id_input UUID,
    display_name_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM app_user_update_v1(user_public_id_input, display_name_input);
END;
$$;

CREATE OR REPLACE FUNCTION app_user_verify_email_v1(
    user_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$DECLARE
    base_message CONSTANT text := 'Failed to verify app user email';
    errcode CONSTANT text := 'P0001';

BEGIN
    IF user_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'user_missing';
    END IF;

    UPDATE app_user
    SET is_email_verified = TRUE
    WHERE user_public_id = user_public_id_input;

    IF NOT FOUND THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'user_not_found';
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION app_user_verify_email(
    user_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM app_user_verify_email_v1(user_public_id_input);
END;
$$;
