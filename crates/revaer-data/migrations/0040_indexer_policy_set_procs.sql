-- Stored procedures for policy set and rule management.

CREATE OR REPLACE FUNCTION policy_set_create_v1(
    actor_user_public_id UUID,
    display_name_input VARCHAR,
    scope_input policy_scope,
    enabled_input BOOLEAN
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to create policy set';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    new_policy_set_id BIGINT;
    new_policy_set_public_id UUID;
    trimmed_display_name VARCHAR(256);
    resolved_enabled BOOLEAN;
    resolved_user_id BIGINT;
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

    IF scope_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'scope_missing';
    END IF;

    resolved_enabled := COALESCE(enabled_input, TRUE);

    IF scope_input IN ('global', 'profile') THEN
        IF actor_role NOT IN ('owner', 'admin') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_unauthorized';
        END IF;
        resolved_user_id := NULL;
    ELSE
        resolved_user_id := actor_user_id;
    END IF;

    IF scope_input = 'profile' AND resolved_enabled THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'profile_policy_set_requires_link';
    END IF;

    IF scope_input = 'global' AND resolved_enabled THEN
        IF EXISTS (
            SELECT 1
            FROM policy_set
            WHERE scope = 'global'
              AND is_enabled = TRUE
              AND deleted_at IS NULL
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'global_policy_set_exists';
        END IF;
    END IF;

    IF scope_input = 'user' AND resolved_enabled THEN
        IF EXISTS (
            SELECT 1
            FROM policy_set
            WHERE scope = 'user'
              AND user_id = resolved_user_id
              AND is_enabled = TRUE
              AND deleted_at IS NULL
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'user_policy_set_exists';
        END IF;
    END IF;

    new_policy_set_public_id := gen_random_uuid();

    INSERT INTO policy_set (
        policy_set_public_id,
        user_id,
        display_name,
        scope,
        is_enabled,
        is_auto_created,
        created_for_search_request_id,
        created_by_user_id,
        updated_by_user_id
    )
    VALUES (
        new_policy_set_public_id,
        resolved_user_id,
        trimmed_display_name,
        scope_input,
        resolved_enabled,
        FALSE,
        NULL,
        actor_user_id,
        actor_user_id
    )
    RETURNING policy_set_id INTO new_policy_set_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'policy_set',
        new_policy_set_id,
        new_policy_set_public_id,
        'create',
        actor_user_id,
        'policy_set_create'
    );

    RETURN new_policy_set_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION policy_set_create(
    actor_user_public_id UUID,
    display_name_input VARCHAR,
    scope_input policy_scope,
    enabled_input BOOLEAN
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN policy_set_create_v1(
        actor_user_public_id,
        display_name_input,
        scope_input,
        enabled_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION policy_set_update_v1(
    actor_user_public_id UUID,
    policy_set_public_id_input UUID,
    display_name_input VARCHAR
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to update policy set';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    policy_set_id_value BIGINT;
    policy_scope_value policy_scope;
    policy_user_id BIGINT;
    policy_deleted_at TIMESTAMPTZ;
    trimmed_display_name VARCHAR(256);
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

    UPDATE policy_set
    SET display_name = trimmed_display_name,
        updated_by_user_id = actor_user_id,
        updated_at = now()
    WHERE policy_set_id = policy_set_id_value;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'policy_set',
        policy_set_id_value,
        policy_set_public_id_input,
        'update',
        actor_user_id,
        'policy_set_update'
    );

    RETURN policy_set_public_id_input;
END;
$$;

CREATE OR REPLACE FUNCTION policy_set_update(
    actor_user_public_id UUID,
    policy_set_public_id_input UUID,
    display_name_input VARCHAR
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN policy_set_update_v1(
        actor_user_public_id,
        policy_set_public_id_input,
        display_name_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION policy_set_enable_v1(
    actor_user_public_id UUID,
    policy_set_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to enable policy set';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    policy_set_id_value BIGINT;
    policy_scope_value policy_scope;
    policy_user_id BIGINT;
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

    IF policy_scope_value = 'profile' THEN
        IF NOT EXISTS (
            SELECT 1
            FROM search_profile_policy_set
            WHERE policy_set_id = policy_set_id_value
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'profile_policy_set_requires_link';
        END IF;
    END IF;

    IF policy_scope_value = 'global' THEN
        IF EXISTS (
            SELECT 1
            FROM policy_set
            WHERE scope = 'global'
              AND is_enabled = TRUE
              AND deleted_at IS NULL
              AND policy_set_id <> policy_set_id_value
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'global_policy_set_exists';
        END IF;
    END IF;

    IF policy_scope_value = 'user' THEN
        IF EXISTS (
            SELECT 1
            FROM policy_set
            WHERE scope = 'user'
              AND user_id = policy_user_id
              AND is_enabled = TRUE
              AND deleted_at IS NULL
              AND policy_set_id <> policy_set_id_value
        ) THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'user_policy_set_exists';
        END IF;
    END IF;

    UPDATE policy_set
    SET is_enabled = TRUE,
        updated_by_user_id = actor_user_id,
        updated_at = now()
    WHERE policy_set_id = policy_set_id_value;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'policy_set',
        policy_set_id_value,
        policy_set_public_id_input,
        'enable',
        actor_user_id,
        'policy_set_enable'
    );
END;
$$;

CREATE OR REPLACE FUNCTION policy_set_enable(
    actor_user_public_id UUID,
    policy_set_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM policy_set_enable_v1(actor_user_public_id, policy_set_public_id_input);
END;
$$;

CREATE OR REPLACE FUNCTION policy_set_disable_v1(
    actor_user_public_id UUID,
    policy_set_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to disable policy set';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    policy_set_id_value BIGINT;
    policy_scope_value policy_scope;
    policy_user_id BIGINT;
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

    UPDATE policy_set
    SET is_enabled = FALSE,
        updated_by_user_id = actor_user_id,
        updated_at = now()
    WHERE policy_set_id = policy_set_id_value;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'policy_set',
        policy_set_id_value,
        policy_set_public_id_input,
        'disable',
        actor_user_id,
        'policy_set_disable'
    );
END;
$$;

CREATE OR REPLACE FUNCTION policy_set_disable(
    actor_user_public_id UUID,
    policy_set_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM policy_set_disable_v1(actor_user_public_id, policy_set_public_id_input);
END;
$$;

CREATE OR REPLACE FUNCTION policy_set_reorder_v1(
    actor_user_public_id UUID,
    ordered_policy_set_public_ids UUID[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to reorder policy sets';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    target_scope policy_scope;
    target_user_id BIGINT;
    id_count INTEGER;
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

    IF ordered_policy_set_public_ids IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_set_ids_missing';
    END IF;

    SELECT count(DISTINCT value)
    INTO id_count
    FROM unnest(ordered_policy_set_public_ids) AS value;

    IF id_count = 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_set_ids_empty';
    END IF;

    SELECT scope, user_id
    INTO target_scope, target_user_id
    FROM policy_set
    WHERE policy_set_public_id = ordered_policy_set_public_ids[1];

    IF target_scope IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_set_not_found';
    END IF;

    SELECT count(*)
    INTO resolved_count
    FROM policy_set
    WHERE policy_set_public_id = ANY(ordered_policy_set_public_ids)
      AND deleted_at IS NULL
      AND scope = target_scope
      AND (
          (target_scope IN ('user', 'request') AND user_id = target_user_id)
          OR target_scope IN ('global', 'profile')
      );

    IF resolved_count <> id_count THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_set_not_found';
    END IF;

    IF target_scope IN ('global', 'profile') THEN
        IF actor_role NOT IN ('owner', 'admin') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_unauthorized';
        END IF;
    ELSIF target_scope IN ('user', 'request') THEN
        IF target_user_id IS NULL OR target_user_id <> actor_user_id THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_unauthorized';
        END IF;
    END IF;

    WITH ordered AS (
        SELECT value AS policy_set_public_id,
               (row_number() OVER (ORDER BY ordinality) * 10) AS new_sort_order
        FROM unnest(ordered_policy_set_public_ids) WITH ORDINALITY AS value
    )
    UPDATE policy_set
    SET sort_order = ordered.new_sort_order,
        updated_by_user_id = actor_user_id,
        updated_at = now()
    FROM ordered
    WHERE policy_set.policy_set_public_id = ordered.policy_set_public_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    SELECT
        'policy_set',
        policy_set_id,
        policy_set_public_id,
        'update',
        actor_user_id,
        'policy_set_reorder'
    FROM policy_set
    WHERE policy_set_public_id = ANY(ordered_policy_set_public_ids);
END;
$$;

CREATE OR REPLACE FUNCTION policy_set_reorder(
    actor_user_public_id UUID,
    ordered_policy_set_public_ids UUID[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM policy_set_reorder_v1(actor_user_public_id, ordered_policy_set_public_ids);
END;
$$;

CREATE OR REPLACE FUNCTION policy_rule_disable_v1(
    actor_user_public_id UUID,
    policy_rule_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to disable policy rule';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    rule_id BIGINT;
    policy_set_id_value BIGINT;
    policy_scope_value policy_scope;
    policy_user_id BIGINT;
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

    IF policy_rule_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_rule_missing';
    END IF;

    SELECT policy_rule_id, policy_set_id
    INTO rule_id, policy_set_id_value
    FROM policy_rule
    WHERE policy_rule_public_id = policy_rule_public_id_input;

    IF rule_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_rule_not_found';
    END IF;

    SELECT scope, user_id, deleted_at
    INTO policy_scope_value, policy_user_id, policy_deleted_at
    FROM policy_set
    WHERE policy_set_id = policy_set_id_value;

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

    UPDATE policy_rule
    SET is_disabled = TRUE,
        updated_by_user_id = actor_user_id,
        updated_at = now()
    WHERE policy_rule_id = rule_id;

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
        rule_id,
        policy_rule_public_id_input,
        'update',
        actor_user_id,
        'policy_rule_disable'
    );
END;
$$;

CREATE OR REPLACE FUNCTION policy_rule_disable(
    actor_user_public_id UUID,
    policy_rule_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM policy_rule_disable_v1(actor_user_public_id, policy_rule_public_id_input);
END;
$$;

CREATE OR REPLACE FUNCTION policy_rule_enable_v1(
    actor_user_public_id UUID,
    policy_rule_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to enable policy rule';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    rule_id BIGINT;
    policy_set_id_value BIGINT;
    policy_scope_value policy_scope;
    policy_user_id BIGINT;
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

    IF policy_rule_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_rule_missing';
    END IF;

    SELECT policy_rule_id, policy_set_id
    INTO rule_id, policy_set_id_value
    FROM policy_rule
    WHERE policy_rule_public_id = policy_rule_public_id_input;

    IF rule_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_rule_not_found';
    END IF;

    SELECT scope, user_id, deleted_at
    INTO policy_scope_value, policy_user_id, policy_deleted_at
    FROM policy_set
    WHERE policy_set_id = policy_set_id_value;

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

    UPDATE policy_rule
    SET is_disabled = FALSE,
        updated_by_user_id = actor_user_id,
        updated_at = now()
    WHERE policy_rule_id = rule_id;

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
        rule_id,
        policy_rule_public_id_input,
        'update',
        actor_user_id,
        'policy_rule_enable'
    );
END;
$$;

CREATE OR REPLACE FUNCTION policy_rule_enable(
    actor_user_public_id UUID,
    policy_rule_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM policy_rule_enable_v1(actor_user_public_id, policy_rule_public_id_input);
END;
$$;

CREATE OR REPLACE FUNCTION policy_rule_reorder_v1(
    actor_user_public_id UUID,
    policy_set_public_id_input UUID,
    ordered_rule_public_ids UUID[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to reorder policy rules';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    policy_set_id_value BIGINT;
    policy_scope_value policy_scope;
    policy_user_id BIGINT;
    policy_deleted_at TIMESTAMPTZ;
    rule_count INTEGER;
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

    IF ordered_rule_public_ids IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_rule_ids_missing';
    END IF;

    SELECT count(DISTINCT value)
    INTO rule_count
    FROM unnest(ordered_rule_public_ids) AS value;

    IF rule_count = 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_rule_ids_empty';
    END IF;

    SELECT count(*)
    INTO resolved_count
    FROM policy_rule
    WHERE policy_rule_public_id = ANY(ordered_rule_public_ids)
      AND policy_set_id = policy_set_id_value;

    IF resolved_count <> rule_count THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'policy_rule_not_found';
    END IF;

    WITH ordered AS (
        SELECT value AS policy_rule_public_id,
               (row_number() OVER (ORDER BY ordinality) * 10) AS new_sort_order
        FROM unnest(ordered_rule_public_ids) WITH ORDINALITY AS value
    )
    UPDATE policy_rule
    SET sort_order = ordered.new_sort_order,
        updated_by_user_id = actor_user_id,
        updated_at = now()
    FROM ordered
    WHERE policy_rule.policy_rule_public_id = ordered.policy_rule_public_id
      AND policy_rule.policy_set_id = policy_set_id_value;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    SELECT
        'policy_rule',
        policy_rule_id,
        policy_rule_public_id,
        'update',
        actor_user_id,
        'policy_rule_reorder'
    FROM policy_rule
    WHERE policy_rule_public_id = ANY(ordered_rule_public_ids)
      AND policy_set_id = policy_set_id_value;
END;
$$;

CREATE OR REPLACE FUNCTION policy_rule_reorder(
    actor_user_public_id UUID,
    policy_set_public_id_input UUID,
    ordered_rule_public_ids UUID[]
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM policy_rule_reorder_v1(
        actor_user_public_id,
        policy_set_public_id_input,
        ordered_rule_public_ids
    );
END;
$$;
