-- Torznab instance enable/disable and soft delete procedures.

CREATE OR REPLACE FUNCTION torznab_instance_enable_disable_v1(
    actor_user_public_id UUID,
    torznab_instance_public_id_input UUID,
    is_enabled_input BOOLEAN
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to update torznab instance';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    instance_deleted_at TIMESTAMPTZ;
    current_is_enabled BOOLEAN;
    audit_action audit_action;
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

    SELECT torznab_instance_id, search_profile_id, is_enabled, deleted_at
    INTO instance_id, profile_id, current_is_enabled, instance_deleted_at
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

    IF is_enabled_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'is_enabled_missing';
    END IF;

    IF current_is_enabled IS DISTINCT FROM is_enabled_input THEN
        IF is_enabled_input THEN
            audit_action := 'enable';
        ELSE
            audit_action := 'disable';
        END IF;
    ELSE
        audit_action := 'update';
    END IF;

    UPDATE torznab_instance
    SET is_enabled = is_enabled_input,
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
        audit_action,
        actor_user_id,
        'torznab_instance_enable_disable'
    );
END;
$$;

CREATE OR REPLACE FUNCTION torznab_instance_enable_disable(
    actor_user_public_id UUID,
    torznab_instance_public_id_input UUID,
    is_enabled_input BOOLEAN
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM torznab_instance_enable_disable_v1(
        actor_user_public_id,
        torznab_instance_public_id_input,
        is_enabled_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION torznab_instance_soft_delete_v1(
    actor_user_public_id UUID,
    torznab_instance_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to delete torznab instance';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    profile_id BIGINT;
    profile_user_id BIGINT;
    profile_deleted_at TIMESTAMPTZ;
    instance_deleted_at TIMESTAMPTZ;
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

    UPDATE torznab_instance
    SET deleted_at = COALESCE(deleted_at, now()),
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
        'soft_delete',
        actor_user_id,
        'torznab_instance_soft_delete'
    );
END;
$$;

CREATE OR REPLACE FUNCTION torznab_instance_soft_delete(
    actor_user_public_id UUID,
    torznab_instance_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM torznab_instance_soft_delete_v1(
        actor_user_public_id,
        torznab_instance_public_id_input
    );
END;
$$;
