-- Secret read procedure.

CREATE OR REPLACE FUNCTION secret_read_v1(
    actor_user_public_id UUID,
    secret_public_id_input UUID
)
RETURNS TABLE (
    secret_type secret_type,
    cipher_text BYTEA,
    key_id VARCHAR
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to read secret';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    secret_is_revoked BOOLEAN;
BEGIN
    IF secret_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_missing';
    END IF;

    IF actor_user_public_id IS NOT NULL THEN
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
    END IF;

    SELECT secret.secret_type, secret.cipher_text, secret.key_id, secret.is_revoked
    INTO secret_type, cipher_text, key_id, secret_is_revoked
    FROM secret
    WHERE secret_public_id = secret_public_id_input;

    IF secret_type IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_not_found';
    END IF;

    IF secret_is_revoked THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_revoked';
    END IF;

    RETURN NEXT;
    RETURN;
END;
$$;

CREATE OR REPLACE FUNCTION secret_read(
    actor_user_public_id UUID,
    secret_public_id_input UUID
)
RETURNS TABLE (
    secret_type secret_type,
    cipher_text BYTEA,
    key_id VARCHAR
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT *
    FROM secret_read_v1(actor_user_public_id, secret_public_id_input);
END;
$$;
