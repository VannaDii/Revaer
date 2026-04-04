-- Conflict resolution procedures.

CREATE OR REPLACE FUNCTION source_metadata_conflict_resolve_v1(
    actor_user_public_id UUID,
    conflict_id_input BIGINT,
    resolution_input conflict_resolution,
    resolution_note_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to resolve conflict';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    conflict_type_value conflict_type;
    conflict_canonical_source_id BIGINT;
    conflict_existing_value VARCHAR(256);
    conflict_incoming_value VARCHAR(256);
    conflict_resolved_at TIMESTAMPTZ;
    source_guid_value VARCHAR(256);
    source_indexer_instance_id BIGINT;
    incoming_trimmed VARCHAR(256);
    tracker_category_value INTEGER;
    tracker_subcategory_value INTEGER;
    tracker_parts TEXT[];
    tracker_part_text TEXT;
    tracker_part_sub TEXT;
    has_tracker_name BOOLEAN;
    has_tracker_category BOOLEAN;
    has_tracker_subcategory BOOLEAN;
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

    IF conflict_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'conflict_missing';
    END IF;

    IF resolution_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'resolution_missing';
    END IF;

    IF resolution_note_input IS NOT NULL AND char_length(resolution_note_input) > 256 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'resolution_note_too_long';
    END IF;

    SELECT conflict_type,
           canonical_torrent_source_id,
           existing_value,
           incoming_value,
           resolved_at
    INTO conflict_type_value,
         conflict_canonical_source_id,
         conflict_existing_value,
         conflict_incoming_value,
         conflict_resolved_at
    FROM source_metadata_conflict
    WHERE source_metadata_conflict_id = conflict_id_input;

    IF conflict_canonical_source_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'conflict_not_found';
    END IF;

    IF conflict_resolved_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'conflict_already_resolved';
    END IF;

    IF resolution_input = 'accepted_incoming' THEN
        incoming_trimmed := trim(conflict_incoming_value);
        IF incoming_trimmed = '' THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'incoming_value_invalid';
        END IF;

        IF conflict_type_value = 'source_guid' THEN
            SELECT source_guid, indexer_instance_id
            INTO source_guid_value, source_indexer_instance_id
            FROM canonical_torrent_source
            WHERE canonical_torrent_source_id = conflict_canonical_source_id;

            IF source_guid_value IS NULL THEN
                IF EXISTS (
                    SELECT 1
                    FROM canonical_torrent_source
                    WHERE indexer_instance_id = source_indexer_instance_id
                      AND source_guid = incoming_trimmed
                ) THEN
                    RAISE EXCEPTION USING
                        ERRCODE = errcode,
                        MESSAGE = base_message,
                        DETAIL = 'source_guid_conflict';
                END IF;

                UPDATE canonical_torrent_source
                SET source_guid = incoming_trimmed,
                    updated_at = now()
                WHERE canonical_torrent_source_id = conflict_canonical_source_id;
            END IF;
        ELSIF conflict_type_value = 'tracker_name' THEN
            SELECT EXISTS (
                SELECT 1
                FROM canonical_torrent_source_attr
                WHERE canonical_torrent_source_id = conflict_canonical_source_id
                  AND attr_key = 'tracker_name'
            )
            INTO has_tracker_name;

            IF has_tracker_name IS FALSE THEN
                INSERT INTO canonical_torrent_source_attr (
                    canonical_torrent_source_id,
                    attr_key,
                    value_text
                )
                VALUES (
                    conflict_canonical_source_id,
                    'tracker_name',
                    incoming_trimmed
                );
            END IF;
        ELSIF conflict_type_value = 'tracker_category' THEN
            tracker_parts := regexp_split_to_array(incoming_trimmed, '[:/]');

            IF array_length(tracker_parts, 1) IS NULL OR array_length(tracker_parts, 1) > 2 THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'incoming_value_invalid';
            END IF;

            tracker_part_text := tracker_parts[1];
            IF tracker_part_text !~ '^[0-9]+$' THEN
                RAISE EXCEPTION USING
                    ERRCODE = errcode,
                    MESSAGE = base_message,
                    DETAIL = 'incoming_value_invalid';
            END IF;

            tracker_category_value := tracker_part_text::INTEGER;
            tracker_subcategory_value := 0;

            IF array_length(tracker_parts, 1) = 2 THEN
                tracker_part_sub := tracker_parts[2];
                IF tracker_part_sub !~ '^[0-9]+$' THEN
                    RAISE EXCEPTION USING
                        ERRCODE = errcode,
                        MESSAGE = base_message,
                        DETAIL = 'incoming_value_invalid';
                END IF;
                tracker_subcategory_value := tracker_part_sub::INTEGER;
            END IF;

            SELECT EXISTS (
                SELECT 1
                FROM canonical_torrent_source_attr
                WHERE canonical_torrent_source_id = conflict_canonical_source_id
                  AND attr_key = 'tracker_category'
            )
            INTO has_tracker_category;

            SELECT EXISTS (
                SELECT 1
                FROM canonical_torrent_source_attr
                WHERE canonical_torrent_source_id = conflict_canonical_source_id
                  AND attr_key = 'tracker_subcategory'
            )
            INTO has_tracker_subcategory;

            IF has_tracker_category IS FALSE THEN
                INSERT INTO canonical_torrent_source_attr (
                    canonical_torrent_source_id,
                    attr_key,
                    value_int
                )
                VALUES (
                    conflict_canonical_source_id,
                    'tracker_category',
                    tracker_category_value
                );
            END IF;

            IF has_tracker_subcategory IS FALSE THEN
                INSERT INTO canonical_torrent_source_attr (
                    canonical_torrent_source_id,
                    attr_key,
                    value_int
                )
                VALUES (
                    conflict_canonical_source_id,
                    'tracker_subcategory',
                    tracker_subcategory_value
                );
            END IF;
        END IF;
    END IF;

    UPDATE source_metadata_conflict
    SET resolved_at = now(),
        resolved_by_user_id = actor_user_id,
        resolution = resolution_input,
        resolution_note = resolution_note_input
    WHERE source_metadata_conflict_id = conflict_id_input;

    INSERT INTO source_metadata_conflict_audit_log (
        conflict_id,
        action,
        actor_user_id,
        occurred_at,
        note
    )
    VALUES (
        conflict_id_input,
        'resolved',
        actor_user_id,
        now(),
        resolution_note_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION source_metadata_conflict_resolve(
    actor_user_public_id UUID,
    conflict_id_input BIGINT,
    resolution_input conflict_resolution,
    resolution_note_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM source_metadata_conflict_resolve_v1(
        actor_user_public_id,
        conflict_id_input,
        resolution_input,
        resolution_note_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION source_metadata_conflict_reopen_v1(
    actor_user_public_id UUID,
    conflict_id_input BIGINT,
    resolution_note_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to reopen conflict';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    conflict_resolved_at TIMESTAMPTZ;
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

    IF conflict_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'conflict_missing';
    END IF;

    IF resolution_note_input IS NOT NULL AND char_length(resolution_note_input) > 256 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'resolution_note_too_long';
    END IF;

    SELECT resolved_at
    INTO conflict_resolved_at
    FROM source_metadata_conflict
    WHERE source_metadata_conflict_id = conflict_id_input;

    IF conflict_resolved_at IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'conflict_not_resolved';
    END IF;

    UPDATE source_metadata_conflict
    SET resolved_at = NULL,
        resolved_by_user_id = NULL,
        resolution = NULL,
        resolution_note = NULL
    WHERE source_metadata_conflict_id = conflict_id_input;

    INSERT INTO source_metadata_conflict_audit_log (
        conflict_id,
        action,
        actor_user_id,
        occurred_at,
        note
    )
    VALUES (
        conflict_id_input,
        'reopened',
        actor_user_id,
        now(),
        resolution_note_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION source_metadata_conflict_reopen(
    actor_user_public_id UUID,
    conflict_id_input BIGINT,
    resolution_note_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM source_metadata_conflict_reopen_v1(
        actor_user_public_id,
        conflict_id_input,
        resolution_note_input
    );
END;
$$;
