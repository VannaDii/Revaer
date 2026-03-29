-- Stored procedure for policy rule creation.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'policy_rule_value_item') THEN
        CREATE TYPE policy_rule_value_item AS (
            value_text VARCHAR,
            value_int INTEGER,
            value_bigint BIGINT,
            value_uuid UUID
        );
    END IF;
END
$$;

CREATE OR REPLACE FUNCTION policy_rule_create_v1(
    actor_user_public_id UUID,
    policy_set_public_id_input UUID,
    rule_type_input policy_rule_type,
    match_field_input policy_match_field,
    match_operator_input policy_match_operator,
    sort_order_input INTEGER,
    match_value_text_input VARCHAR,
    match_value_int_input INTEGER,
    match_value_uuid_input UUID,
    value_set_items_input policy_rule_value_item[],
    action_input policy_action,
    severity_input policy_severity,
    is_case_insensitive_input BOOLEAN,
    rationale_input VARCHAR,
    expires_at_input TIMESTAMPTZ
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to create policy rule';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    policy_set_id_value BIGINT;
    policy_scope_value policy_scope;
    policy_user_id BIGINT;
    policy_deleted_at TIMESTAMPTZ;
    new_rule_id BIGINT;
    new_rule_public_id UUID;
    resolved_sort_order INTEGER;
    resolved_is_case_insensitive BOOLEAN;
    resolved_match_value_text VARCHAR(512);
    value_set_id_value BIGINT;
    value_set_type_value value_set_type;
    item policy_rule_value_item;
    item_count INTEGER;
    seen_texts TEXT[];
    seen_ints INTEGER[];
    seen_bigints BIGINT[];
    seen_uuids UUID[];
    normalized_text TEXT;
    non_null_count INTEGER;
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

    IF policy_set_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_set_missing';
    END IF;

    SELECT policy_set_id, scope, user_id, deleted_at
    INTO policy_set_id_value, policy_scope_value, policy_user_id, policy_deleted_at
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

    IF policy_scope_value IN ('global', 'profile') THEN
        IF actor_role NOT IN ('owner', 'admin') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_unauthorized';
        END IF;
    ELSIF policy_scope_value IN ('user', 'request') THEN
        IF policy_user_id IS NULL OR policy_user_id <> actor_user_id THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_unauthorized';
        END IF;
    END IF;

    IF rule_type_input IS NULL OR match_field_input IS NULL OR match_operator_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'rule_definition_missing';
    END IF;

    IF action_input IS NULL OR severity_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'rule_action_missing';
    END IF;

    resolved_sort_order := COALESCE(sort_order_input, 1000);
    resolved_is_case_insensitive := COALESCE(is_case_insensitive_input, TRUE);

    IF rationale_input IS NOT NULL AND char_length(rationale_input) > 1024 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'rationale_too_long';
    END IF;

    IF rule_type_input = 'block_infohash_v1' AND match_field_input <> 'infohash_v1' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'match_field_invalid';
    END IF;

    IF rule_type_input = 'block_infohash_v2' AND match_field_input <> 'infohash_v2' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'match_field_invalid';
    END IF;

    IF rule_type_input = 'block_magnet' AND match_field_input <> 'magnet_hash' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'match_field_invalid';
    END IF;

    IF rule_type_input = 'block_infohash_v1'
        OR rule_type_input = 'block_infohash_v2'
        OR rule_type_input = 'block_magnet' THEN
        IF action_input <> 'drop_canonical' OR severity_input <> 'hard' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'action_invalid';
        END IF;
    END IF;

    IF rule_type_input = 'require_trust_tier_min' THEN
        IF match_field_input <> 'trust_tier_rank'
            OR match_operator_input <> 'eq'
            OR match_value_int_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'match_value_invalid';
        END IF;
    END IF;

    IF match_operator_input = 'in_set' THEN
        IF match_value_text_input IS NOT NULL
            OR match_value_int_input IS NOT NULL
            OR match_value_uuid_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'match_value_invalid';
        END IF;

        IF value_set_items_input IS NULL
            OR array_length(value_set_items_input, 1) IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'value_set_missing';
        END IF;
    ELSE
        IF value_set_items_input IS NOT NULL
            AND array_length(value_set_items_input, 1) IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'value_set_not_allowed';
        END IF;
    END IF;

    resolved_match_value_text := NULL;

    IF match_field_input IN ('infohash_v1', 'infohash_v2', 'magnet_hash') THEN
        IF match_operator_input NOT IN ('eq', 'in_set') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'match_operator_invalid';
        END IF;
        IF match_value_int_input IS NOT NULL OR match_value_uuid_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'match_value_invalid';
        END IF;

        IF match_operator_input = 'eq' THEN
            IF match_value_text_input IS NULL THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'match_value_invalid';
            END IF;
            resolved_match_value_text := lower(trim(match_value_text_input));
            IF resolved_match_value_text = '' THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'match_value_invalid';
            END IF;
            IF match_field_input = 'infohash_v1' AND resolved_match_value_text !~ '^[0-9a-f]{40}$' THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'match_value_invalid';
            END IF;
            IF match_field_input IN ('infohash_v2', 'magnet_hash')
                AND resolved_match_value_text !~ '^[0-9a-f]{64}$' THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'match_value_invalid';
            END IF;
        END IF;
        value_set_type_value := 'text';
    ELSIF match_field_input = 'indexer_instance_public_id' THEN
        IF match_operator_input NOT IN ('eq', 'in_set') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'match_operator_invalid';
        END IF;
        IF match_value_text_input IS NOT NULL OR match_value_int_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'match_value_invalid';
        END IF;
        IF match_operator_input = 'eq' THEN
            IF match_value_uuid_input IS NULL THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'match_value_invalid';
            END IF;
        END IF;
        value_set_type_value := 'uuid';
    ELSIF match_field_input IN ('media_domain_key', 'trust_tier_key') THEN
        IF match_operator_input NOT IN ('eq', 'in_set') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'match_operator_invalid';
        END IF;
        IF match_value_int_input IS NOT NULL OR match_value_uuid_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'match_value_invalid';
        END IF;
        IF match_operator_input = 'eq' THEN
            IF match_value_text_input IS NULL THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'match_value_invalid';
            END IF;
            resolved_match_value_text := lower(trim(match_value_text_input));
            IF resolved_match_value_text = '' THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'match_value_invalid';
            END IF;
        END IF;
        value_set_type_value := 'text';
    ELSIF match_field_input = 'trust_tier_rank' THEN
        IF match_operator_input NOT IN ('eq', 'in_set') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'match_operator_invalid';
        END IF;
        IF match_value_text_input IS NOT NULL OR match_value_uuid_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'match_value_invalid';
        END IF;
        IF match_operator_input = 'eq' AND match_value_int_input IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'match_value_invalid';
        END IF;
        value_set_type_value := 'int';
    ELSE
        IF match_operator_input NOT IN ('eq', 'contains', 'regex', 'starts_with', 'ends_with', 'in_set') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'match_operator_invalid';
        END IF;
        IF match_value_int_input IS NOT NULL OR match_value_uuid_input IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'match_value_invalid';
        END IF;
        IF match_operator_input = 'eq'
            OR match_operator_input = 'contains'
            OR match_operator_input = 'regex'
            OR match_operator_input = 'starts_with'
            OR match_operator_input = 'ends_with' THEN
            IF match_value_text_input IS NULL THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'match_value_invalid';
            END IF;
            resolved_match_value_text := trim(match_value_text_input);
            IF resolved_match_value_text = '' THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'match_value_invalid';
            END IF;
            IF char_length(resolved_match_value_text) > 512 THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'match_value_invalid';
            END IF;
        END IF;
        value_set_type_value := 'text';
    END IF;

    new_rule_public_id := gen_random_uuid();

    INSERT INTO policy_rule (
        policy_set_id,
        policy_rule_public_id,
        rule_type,
        match_field,
        match_operator,
        sort_order,
        match_value_text,
        match_value_int,
        match_value_uuid,
        action,
        severity,
        is_case_insensitive,
        is_disabled,
        rationale,
        expires_at,
        immutable_flag,
        created_by_user_id,
        updated_by_user_id
    )
    VALUES (
        policy_set_id_value,
        new_rule_public_id,
        rule_type_input,
        match_field_input,
        match_operator_input,
        resolved_sort_order,
        resolved_match_value_text,
        match_value_int_input,
        match_value_uuid_input,
        action_input,
        severity_input,
        resolved_is_case_insensitive,
        FALSE,
        rationale_input,
        expires_at_input,
        TRUE,
        actor_user_id,
        actor_user_id
    )
    RETURNING policy_rule_id INTO new_rule_id;

    IF match_operator_input = 'in_set' THEN
        INSERT INTO policy_rule_value_set (
            policy_rule_id,
            value_set_type
        )
        VALUES (
            new_rule_id,
            value_set_type_value
        )
        RETURNING value_set_id INTO value_set_id_value;

        UPDATE policy_rule
        SET value_set_id = value_set_id_value
        WHERE policy_rule_id = new_rule_id;

        item_count := 0;
        seen_texts := ARRAY[]::TEXT[];
        seen_ints := ARRAY[]::INTEGER[];
        seen_bigints := ARRAY[]::BIGINT[];
        seen_uuids := ARRAY[]::UUID[];

        FOREACH item IN ARRAY value_set_items_input LOOP
            item_count := item_count + 1;
            IF item_count > 100 THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'value_set_too_large';
            END IF;

            non_null_count := (item.value_text IS NOT NULL)::INT
                + (item.value_int IS NOT NULL)::INT
                + (item.value_bigint IS NOT NULL)::INT
                + (item.value_uuid IS NOT NULL)::INT;

            IF non_null_count <> 1 THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'value_set_item_invalid';
            END IF;

            IF value_set_type_value = 'text' THEN
                IF item.value_text IS NULL THEN
                    RAISE EXCEPTION USING
                        ERRCODE = errcode,
                        MESSAGE = base_message,
                        DETAIL = 'value_set_item_invalid';
                END IF;
                normalized_text := lower(trim(item.value_text));
                IF normalized_text = '' OR char_length(normalized_text) > 256 THEN
                    RAISE EXCEPTION USING
                        ERRCODE = errcode,
                        MESSAGE = base_message,
                        DETAIL = 'value_set_item_invalid';
                END IF;
                IF normalized_text = ANY(seen_texts) THEN
                    RAISE EXCEPTION USING
                        ERRCODE = errcode,
                        MESSAGE = base_message,
                        DETAIL = 'value_set_duplicate';
                END IF;
                seen_texts := array_append(seen_texts, normalized_text);

                INSERT INTO policy_rule_value_set_item (
                    value_set_id,
                    value_text
                )
                VALUES (
                    value_set_id_value,
                    normalized_text
                );
            ELSIF value_set_type_value = 'int' THEN
                IF item.value_int IS NULL THEN
                    RAISE EXCEPTION USING
                        ERRCODE = errcode,
                        MESSAGE = base_message,
                        DETAIL = 'value_set_item_invalid';
                END IF;
                IF item.value_int = ANY(seen_ints) THEN
                    RAISE EXCEPTION USING
                        ERRCODE = errcode,
                        MESSAGE = base_message,
                        DETAIL = 'value_set_duplicate';
                END IF;
                seen_ints := array_append(seen_ints, item.value_int);

                INSERT INTO policy_rule_value_set_item (
                    value_set_id,
                    value_int
                )
                VALUES (
                    value_set_id_value,
                    item.value_int
                );
            ELSIF value_set_type_value = 'bigint' THEN
                IF item.value_bigint IS NULL THEN
                    RAISE EXCEPTION USING
                        ERRCODE = errcode,
                        MESSAGE = base_message,
                        DETAIL = 'value_set_item_invalid';
                END IF;
                IF item.value_bigint = ANY(seen_bigints) THEN
                    RAISE EXCEPTION USING
                        ERRCODE = errcode,
                        MESSAGE = base_message,
                        DETAIL = 'value_set_duplicate';
                END IF;
                seen_bigints := array_append(seen_bigints, item.value_bigint);

                INSERT INTO policy_rule_value_set_item (
                    value_set_id,
                    value_bigint
                )
                VALUES (
                    value_set_id_value,
                    item.value_bigint
                );
            ELSIF value_set_type_value = 'uuid' THEN
                IF item.value_uuid IS NULL THEN
                    RAISE EXCEPTION USING
                        ERRCODE = errcode,
                        MESSAGE = base_message,
                        DETAIL = 'value_set_item_invalid';
                END IF;
                IF item.value_uuid = ANY(seen_uuids) THEN
                    RAISE EXCEPTION USING
                        ERRCODE = errcode,
                        MESSAGE = base_message,
                        DETAIL = 'value_set_duplicate';
                END IF;
                seen_uuids := array_append(seen_uuids, item.value_uuid);

                INSERT INTO policy_rule_value_set_item (
                    value_set_id,
                    value_uuid
                )
                VALUES (
                    value_set_id_value,
                    item.value_uuid
                );
            END IF;
        END LOOP;
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
        'policy_rule',
        new_rule_id,
        new_rule_public_id,
        'create',
        actor_user_id,
        'policy_rule_create'
    );

    RETURN new_rule_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION policy_rule_create(
    actor_user_public_id UUID,
    policy_set_public_id_input UUID,
    rule_type_input policy_rule_type,
    match_field_input policy_match_field,
    match_operator_input policy_match_operator,
    sort_order_input INTEGER,
    match_value_text_input VARCHAR,
    match_value_int_input INTEGER,
    match_value_uuid_input UUID,
    value_set_items_input policy_rule_value_item[],
    action_input policy_action,
    severity_input policy_severity,
    is_case_insensitive_input BOOLEAN,
    rationale_input VARCHAR,
    expires_at_input TIMESTAMPTZ
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN policy_rule_create_v1(
        actor_user_public_id,
        policy_set_public_id_input,
        rule_type_input,
        match_field_input,
        match_operator_input,
        sort_order_input,
        match_value_text_input,
        match_value_int_input,
        match_value_uuid_input,
        value_set_items_input,
        action_input,
        severity_input,
        is_case_insensitive_input,
        rationale_input,
        expires_at_input
    );
END;
$$;
