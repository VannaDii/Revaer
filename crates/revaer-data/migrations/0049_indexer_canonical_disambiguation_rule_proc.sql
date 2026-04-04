-- Canonical disambiguation rule creation procedure.

CREATE OR REPLACE FUNCTION canonical_disambiguation_rule_create_v1(
    actor_user_public_id UUID,
    rule_type_input disambiguation_rule_type,
    identity_left_type_input disambiguation_identity_type,
    identity_left_value_text_input VARCHAR,
    identity_left_value_uuid_input UUID,
    identity_right_type_input disambiguation_identity_type,
    identity_right_value_text_input VARCHAR,
    identity_right_value_uuid_input UUID,
    reason_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to create disambiguation rule';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    left_text_norm VARCHAR(64);
    right_text_norm VARCHAR(64);
    left_uuid_value UUID;
    right_uuid_value UUID;
    left_type_value disambiguation_identity_type;
    right_type_value disambiguation_identity_type;
    rule_id BIGINT;
    should_swap BOOLEAN;
    temp_text VARCHAR(64);
    temp_uuid UUID;
    temp_type disambiguation_identity_type;
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

    IF rule_type_input IS NULL
        OR identity_left_type_input IS NULL
        OR identity_right_type_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'rule_definition_missing';
    END IF;

    IF reason_input IS NOT NULL AND char_length(reason_input) > 256 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'reason_too_long';
    END IF;

    left_type_value := identity_left_type_input;
    right_type_value := identity_right_type_input;

    left_text_norm := NULL;
    right_text_norm := NULL;
    left_uuid_value := NULL;
    right_uuid_value := NULL;

    IF left_type_value = 'canonical_public_id' THEN
        IF identity_left_value_text_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'left_value_invalid';
        END IF;
        IF identity_left_value_uuid_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'left_value_missing';
        END IF;
        left_uuid_value := identity_left_value_uuid_input;
    ELSE
        IF identity_left_value_uuid_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'left_value_invalid';
        END IF;
        IF identity_left_value_text_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'left_value_missing';
        END IF;
        left_text_norm := lower(trim(identity_left_value_text_input));
        IF left_text_norm = '' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'left_value_invalid';
        END IF;
        IF char_length(left_text_norm) > 64 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'left_value_invalid';
        END IF;
    END IF;

    IF right_type_value = 'canonical_public_id' THEN
        IF identity_right_value_text_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'right_value_invalid';
        END IF;
        IF identity_right_value_uuid_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'right_value_missing';
        END IF;
        right_uuid_value := identity_right_value_uuid_input;
    ELSE
        IF identity_right_value_uuid_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'right_value_invalid';
        END IF;
        IF identity_right_value_text_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'right_value_missing';
        END IF;
        right_text_norm := lower(trim(identity_right_value_text_input));
        IF right_text_norm = '' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'right_value_invalid';
        END IF;
        IF char_length(right_text_norm) > 64 THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'right_value_invalid';
        END IF;
    END IF;

    IF left_type_value IN ('infohash_v1', 'infohash_v2', 'magnet_hash') THEN
        IF left_text_norm IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'left_value_invalid';
        END IF;
        IF left_type_value = 'infohash_v1' AND left_text_norm !~ '^[0-9a-f]{40}$' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'left_value_invalid';
        END IF;
        IF left_type_value IN ('infohash_v2', 'magnet_hash')
            AND left_text_norm !~ '^[0-9a-f]{64}$' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'left_value_invalid';
        END IF;
    END IF;

    IF right_type_value IN ('infohash_v1', 'infohash_v2', 'magnet_hash') THEN
        IF right_text_norm IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'right_value_invalid';
        END IF;
        IF right_type_value = 'infohash_v1' AND right_text_norm !~ '^[0-9a-f]{40}$' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'right_value_invalid';
        END IF;
        IF right_type_value IN ('infohash_v2', 'magnet_hash')
            AND right_text_norm !~ '^[0-9a-f]{64}$' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'right_value_invalid';
        END IF;
    END IF;

    IF ROW(left_type_value, left_text_norm, left_uuid_value)
        = ROW(right_type_value, right_text_norm, right_uuid_value) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'rule_identity_equal';
    END IF;

    should_swap := ROW(left_type_value, left_text_norm, left_uuid_value)
        > ROW(right_type_value, right_text_norm, right_uuid_value);

    IF should_swap THEN
        temp_type := left_type_value;
        left_type_value := right_type_value;
        right_type_value := temp_type;

        temp_text := left_text_norm;
        left_text_norm := right_text_norm;
        right_text_norm := temp_text;

        temp_uuid := left_uuid_value;
        left_uuid_value := right_uuid_value;
        right_uuid_value := temp_uuid;
    END IF;

    IF EXISTS (
        SELECT 1
        FROM canonical_disambiguation_rule
        WHERE identity_left_type = left_type_value
          AND identity_left_value_text IS NOT DISTINCT FROM left_text_norm
          AND identity_left_value_uuid IS NOT DISTINCT FROM left_uuid_value
          AND identity_right_type = right_type_value
          AND identity_right_value_text IS NOT DISTINCT FROM right_text_norm
          AND identity_right_value_uuid IS NOT DISTINCT FROM right_uuid_value
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'rule_exists';
    END IF;

    INSERT INTO canonical_disambiguation_rule (
        created_by_user_id,
        rule_type,
        identity_left_type,
        identity_left_value_text,
        identity_left_value_uuid,
        identity_right_type,
        identity_right_value_text,
        identity_right_value_uuid,
        reason
    )
    VALUES (
        actor_user_id,
        rule_type_input,
        left_type_value,
        left_text_norm,
        left_uuid_value,
        right_type_value,
        right_text_norm,
        right_uuid_value,
        reason_input
    )
    RETURNING canonical_disambiguation_rule_id INTO rule_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'canonical_disambiguation_rule',
        rule_id,
        NULL,
        'create',
        actor_user_id,
        'canonical_disambiguation_rule_create'
    );
END;
$$;

CREATE OR REPLACE FUNCTION canonical_disambiguation_rule_create(
    actor_user_public_id UUID,
    rule_type_input disambiguation_rule_type,
    identity_left_type_input disambiguation_identity_type,
    identity_left_value_text_input VARCHAR,
    identity_left_value_uuid_input UUID,
    identity_right_type_input disambiguation_identity_type,
    identity_right_value_text_input VARCHAR,
    identity_right_value_uuid_input UUID,
    reason_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM canonical_disambiguation_rule_create_v1(
        actor_user_public_id,
        rule_type_input,
        identity_left_type_input,
        identity_left_value_text_input,
        identity_left_value_uuid_input,
        identity_right_type_input,
        identity_right_value_text_input,
        identity_right_value_uuid_input,
        reason_input
    );
END;
$$;
