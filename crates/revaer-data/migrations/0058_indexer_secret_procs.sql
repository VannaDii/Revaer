-- Secret management procedures.

CREATE OR REPLACE FUNCTION secret_create_v1(
    actor_user_public_id UUID,
    secret_type_input secret_type,
    plaintext_value_input VARCHAR
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to create secret';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    secret_public_id UUID;
    secret_id_value BIGINT;
    key_id_value TEXT;
    secret_key_value TEXT;
    cipher_value BYTEA;
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

    IF secret_type_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_type_missing';
    END IF;

    IF plaintext_value_input IS NULL OR btrim(plaintext_value_input) = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_value_missing';
    END IF;

    key_id_value := current_setting('revaer.secret_key_id', true);
    secret_key_value := current_setting('revaer.secret_key', true);

    IF key_id_value IS NULL OR btrim(key_id_value) = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_key_missing';
    END IF;

    IF char_length(key_id_value) > 128 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_key_invalid';
    END IF;

    IF secret_key_value IS NULL OR btrim(secret_key_value) = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_key_missing';
    END IF;

    cipher_value := pgp_sym_encrypt(plaintext_value_input, secret_key_value, 'cipher-algo=aes256');

    secret_public_id := gen_random_uuid();
    INSERT INTO secret (
        secret_public_id,
        secret_type,
        cipher_text,
        key_id
    )
    VALUES (
        secret_public_id,
        secret_type_input,
        cipher_value,
        key_id_value
    )
    RETURNING secret_id INTO secret_id_value;

    INSERT INTO secret_audit_log (
        secret_id,
        action,
        actor_user_id,
        detail
    )
    VALUES (
        secret_id_value,
        'create',
        actor_user_id,
        'secret_create'
    );

    RETURN secret_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION secret_create(
    actor_user_public_id UUID,
    secret_type_input secret_type,
    plaintext_value_input VARCHAR
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN secret_create_v1(actor_user_public_id, secret_type_input, plaintext_value_input);
END;
$$;

CREATE OR REPLACE FUNCTION secret_rotate_v1(
    actor_user_public_id UUID,
    secret_public_id_input UUID,
    plaintext_value_input VARCHAR
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to rotate secret';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    secret_id_value BIGINT;
    key_id_value TEXT;
    secret_key_value TEXT;
    cipher_value BYTEA;
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

    IF secret_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_missing';
    END IF;

    IF plaintext_value_input IS NULL OR btrim(plaintext_value_input) = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_value_missing';
    END IF;

    SELECT secret_id
    INTO secret_id_value
    FROM secret
    WHERE secret_public_id = secret_public_id_input
      AND is_revoked = FALSE;

    IF secret_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_not_found';
    END IF;

    key_id_value := current_setting('revaer.secret_key_id', true);
    secret_key_value := current_setting('revaer.secret_key', true);

    IF key_id_value IS NULL OR btrim(key_id_value) = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_key_missing';
    END IF;

    IF char_length(key_id_value) > 128 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_key_invalid';
    END IF;

    IF secret_key_value IS NULL OR btrim(secret_key_value) = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_key_missing';
    END IF;

    cipher_value := pgp_sym_encrypt(plaintext_value_input, secret_key_value, 'cipher-algo=aes256');

    UPDATE secret
    SET cipher_text = cipher_value,
        rotated_at = now(),
        key_id = key_id_value
    WHERE secret_id = secret_id_value;

    INSERT INTO secret_audit_log (
        secret_id,
        action,
        actor_user_id,
        detail
    )
    VALUES (
        secret_id_value,
        'rotate',
        actor_user_id,
        'secret_rotate'
    );

    RETURN secret_public_id_input;
END;
$$;

CREATE OR REPLACE FUNCTION secret_rotate(
    actor_user_public_id UUID,
    secret_public_id_input UUID,
    plaintext_value_input VARCHAR
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN secret_rotate_v1(actor_user_public_id, secret_public_id_input, plaintext_value_input);
END;
$$;

CREATE OR REPLACE FUNCTION secret_revoke_v1(
    actor_user_public_id UUID,
    secret_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to revoke secret';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    secret_id_value BIGINT;
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

    IF secret_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_missing';
    END IF;

    SELECT secret_id
    INTO secret_id_value
    FROM secret
    WHERE secret_public_id = secret_public_id_input;

    IF secret_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_not_found';
    END IF;

    UPDATE secret
    SET is_revoked = TRUE
    WHERE secret_id = secret_id_value;

    INSERT INTO secret_audit_log (
        secret_id,
        action,
        actor_user_id,
        detail
    )
    VALUES (
        secret_id_value,
        'revoke',
        actor_user_id,
        'secret_revoke'
    );
END;
$$;

CREATE OR REPLACE FUNCTION secret_revoke(
    actor_user_public_id UUID,
    secret_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM secret_revoke_v1(actor_user_public_id, secret_public_id_input);
END;
$$;
