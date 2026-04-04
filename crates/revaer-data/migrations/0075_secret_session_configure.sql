-- Configure session secrets for secret encryption procedures.

CREATE OR REPLACE FUNCTION secret_session_configure_v1(
    actor_user_public_id UUID,
    secret_key_id_input VARCHAR,
    secret_key_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to configure secret session';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    trimmed_key_id VARCHAR(128);
    trimmed_secret VARCHAR(1024);
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

    IF secret_key_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_key_id_missing';
    END IF;

    trimmed_key_id := trim(secret_key_id_input);

    IF trimmed_key_id = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_key_id_missing';
    END IF;

    IF char_length(trimmed_key_id) > 128 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_key_id_invalid';
    END IF;

    IF secret_key_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_key_missing';
    END IF;

    trimmed_secret := trim(secret_key_input);

    IF trimmed_secret = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_key_missing';
    END IF;

    IF char_length(trimmed_secret) > 1024 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_key_invalid';
    END IF;

    PERFORM set_config('revaer.secret_key_id', trimmed_key_id, false);
    PERFORM set_config('revaer.secret_key', trimmed_secret, false);
END;
$$;

CREATE OR REPLACE FUNCTION secret_session_configure(
    actor_user_public_id UUID,
    secret_key_id_input VARCHAR,
    secret_key_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM secret_session_configure_v1(
        actor_user_public_id,
        secret_key_id_input,
        secret_key_input
    );
END;
$$;
