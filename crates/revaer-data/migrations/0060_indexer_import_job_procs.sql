-- Import job procedures.

CREATE OR REPLACE FUNCTION import_job_create_v1(
    actor_user_public_id UUID,
    source_input import_source,
    is_dry_run_input BOOLEAN,
    target_search_profile_public_id_input UUID,
    target_torznab_instance_public_id_input UUID
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to create import job';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    target_search_profile_id_value BIGINT;
    target_torznab_instance_id_value BIGINT;
    job_public_id UUID;
    dry_run_value BOOLEAN;
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

    IF source_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'source_missing';
    END IF;

    IF target_search_profile_public_id_input IS NOT NULL THEN
        SELECT search_profile_id
        INTO target_search_profile_id_value
        FROM search_profile
        WHERE search_profile_public_id = target_search_profile_public_id_input
          AND deleted_at IS NULL;

        IF target_search_profile_id_value IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'search_profile_not_found';
        END IF;
    END IF;

    IF target_torznab_instance_public_id_input IS NOT NULL THEN
        SELECT torznab_instance_id
        INTO target_torznab_instance_id_value
        FROM torznab_instance
        WHERE torznab_instance_public_id = target_torznab_instance_public_id_input
          AND deleted_at IS NULL;

        IF target_torznab_instance_id_value IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'torznab_instance_not_found';
        END IF;
    END IF;

    dry_run_value := COALESCE(is_dry_run_input, FALSE);
    job_public_id := gen_random_uuid();

    INSERT INTO import_job (
        import_job_public_id,
        target_search_profile_id,
        target_torznab_instance_id,
        created_by_user_id,
        source,
        is_dry_run,
        status
    )
    VALUES (
        job_public_id,
        target_search_profile_id_value,
        target_torznab_instance_id_value,
        actor_user_id,
        source_input,
        dry_run_value,
        'pending'
    );

    RETURN job_public_id;
END;
$$;

CREATE OR REPLACE FUNCTION import_job_create(
    actor_user_public_id UUID,
    source_input import_source,
    is_dry_run_input BOOLEAN,
    target_search_profile_public_id_input UUID,
    target_torznab_instance_public_id_input UUID
)
RETURNS UUID
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN import_job_create_v1(
        actor_user_public_id,
        source_input,
        is_dry_run_input,
        target_search_profile_public_id_input,
        target_torznab_instance_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION import_job_run_prowlarr_api_v1(
    import_job_public_id_input UUID,
    prowlarr_url_input VARCHAR,
    prowlarr_api_key_secret_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to start import job';
    errcode CONSTANT text := 'P0001';
    job_id BIGINT;
    job_source import_source;
    job_status import_job_status;
    secret_id_value BIGINT;
    trimmed_url VARCHAR(2048);
    config_detail VARCHAR(1024);
BEGIN
    IF import_job_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'import_job_missing';
    END IF;

    SELECT import_job_id, source, status
    INTO job_id, job_source, job_status
    FROM import_job
    WHERE import_job_public_id = import_job_public_id_input;

    IF job_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'import_job_not_found';
    END IF;

    IF job_source <> 'prowlarr_api' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'import_source_mismatch';
    END IF;

    IF job_status NOT IN ('pending', 'failed') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'import_job_not_startable';
    END IF;

    IF prowlarr_url_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'prowlarr_url_missing';
    END IF;

    trimmed_url := btrim(prowlarr_url_input);
    IF trimmed_url = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'prowlarr_url_missing';
    END IF;

    IF char_length(trimmed_url) > 2048 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'prowlarr_url_too_long';
    END IF;

    IF prowlarr_api_key_secret_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_missing';
    END IF;

    SELECT secret_id
    INTO secret_id_value
    FROM secret
    WHERE secret_public_id = prowlarr_api_key_secret_public_id_input
      AND is_revoked = FALSE;

    IF secret_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'secret_not_found';
    END IF;

    config_detail := 'prowlarr_url=' || trimmed_url || ';secret_public_id='
        || prowlarr_api_key_secret_public_id_input::TEXT;

    IF char_length(config_detail) > 1024 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'config_too_long';
    END IF;

    UPDATE import_job
    SET status = 'running',
        started_at = now(),
        error_detail = config_detail
    WHERE import_job_id = job_id;
END;
$$;

CREATE OR REPLACE FUNCTION import_job_run_prowlarr_api(
    import_job_public_id_input UUID,
    prowlarr_url_input VARCHAR,
    prowlarr_api_key_secret_public_id_input UUID
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM import_job_run_prowlarr_api_v1(
        import_job_public_id_input,
        prowlarr_url_input,
        prowlarr_api_key_secret_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION import_job_run_prowlarr_backup_v1(
    import_job_public_id_input UUID,
    backup_blob_ref_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to start import job';
    errcode CONSTANT text := 'P0001';
    job_id BIGINT;
    job_source import_source;
    job_status import_job_status;
    trimmed_ref VARCHAR(1024);
    config_detail VARCHAR(1024);
BEGIN
    IF import_job_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'import_job_missing';
    END IF;

    SELECT import_job_id, source, status
    INTO job_id, job_source, job_status
    FROM import_job
    WHERE import_job_public_id = import_job_public_id_input;

    IF job_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'import_job_not_found';
    END IF;

    IF job_source <> 'prowlarr_backup' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'import_source_mismatch';
    END IF;

    IF job_status NOT IN ('pending', 'failed') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'import_job_not_startable';
    END IF;

    IF backup_blob_ref_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'backup_blob_missing';
    END IF;

    trimmed_ref := btrim(backup_blob_ref_input);
    IF trimmed_ref = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'backup_blob_missing';
    END IF;

    IF char_length(trimmed_ref) > 1024 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'backup_blob_too_long';
    END IF;

    config_detail := 'backup_blob_ref=' || trimmed_ref;

    IF char_length(config_detail) > 1024 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'config_too_long';
    END IF;

    UPDATE import_job
    SET status = 'running',
        started_at = now(),
        error_detail = config_detail
    WHERE import_job_id = job_id;
END;
$$;

CREATE OR REPLACE FUNCTION import_job_run_prowlarr_backup(
    import_job_public_id_input UUID,
    backup_blob_ref_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM import_job_run_prowlarr_backup_v1(
        import_job_public_id_input,
        backup_blob_ref_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION import_job_get_status_v1(
    import_job_public_id_input UUID
)
RETURNS TABLE(
    status import_job_status,
    result_total INTEGER,
    result_imported_ready INTEGER,
    result_imported_needs_secret INTEGER,
    result_imported_test_failed INTEGER,
    result_unmapped_definition INTEGER,
    result_skipped_duplicate INTEGER
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to fetch import status';
    errcode CONSTANT text := 'P0001';
    job_id BIGINT;
BEGIN
    IF import_job_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'import_job_missing';
    END IF;

    SELECT import_job_id, import_job.status
    INTO job_id, status
    FROM import_job
    WHERE import_job_public_id = import_job_public_id_input;

    IF job_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'import_job_not_found';
    END IF;

    SELECT COUNT(*),
           COUNT(*) FILTER (WHERE status = 'imported_ready'),
           COUNT(*) FILTER (WHERE status = 'imported_needs_secret'),
           COUNT(*) FILTER (WHERE status = 'imported_test_failed'),
           COUNT(*) FILTER (WHERE status = 'unmapped_definition'),
           COUNT(*) FILTER (WHERE status = 'skipped_duplicate')
    INTO result_total,
         result_imported_ready,
         result_imported_needs_secret,
         result_imported_test_failed,
         result_unmapped_definition,
         result_skipped_duplicate
    FROM import_indexer_result
    WHERE import_job_id = job_id;

    RETURN NEXT;
END;
$$;

CREATE OR REPLACE FUNCTION import_job_get_status(
    import_job_public_id_input UUID
)
RETURNS TABLE(
    status import_job_status,
    result_total INTEGER,
    result_imported_ready INTEGER,
    result_imported_needs_secret INTEGER,
    result_imported_test_failed INTEGER,
    result_unmapped_definition INTEGER,
    result_skipped_duplicate INTEGER
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT * FROM import_job_get_status_v1(import_job_public_id_input);
END;
$$;

CREATE OR REPLACE FUNCTION import_job_list_results_v1(
    import_job_public_id_input UUID
)
RETURNS TABLE(
    prowlarr_identifier VARCHAR,
    upstream_slug VARCHAR,
    indexer_instance_public_id UUID,
    status import_indexer_result_status,
    detail VARCHAR,
    created_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to fetch import results';
    errcode CONSTANT text := 'P0001';
    job_id BIGINT;
BEGIN
    IF import_job_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'import_job_missing';
    END IF;

    SELECT import_job_id
    INTO job_id
    FROM import_job
    WHERE import_job_public_id = import_job_public_id_input;

    IF job_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'import_job_not_found';
    END IF;

    RETURN QUERY
    SELECT r.prowlarr_identifier,
           r.upstream_slug,
           i.indexer_instance_public_id,
           r.status,
           r.detail,
           r.created_at
    FROM import_indexer_result r
    LEFT JOIN indexer_instance i
        ON i.indexer_instance_id = r.indexer_instance_id
    WHERE r.import_job_id = job_id
    ORDER BY r.created_at ASC, r.import_indexer_result_id ASC;
END;
$$;

CREATE OR REPLACE FUNCTION import_job_list_results(
    import_job_public_id_input UUID
)
RETURNS TABLE(
    prowlarr_identifier VARCHAR,
    upstream_slug VARCHAR,
    indexer_instance_public_id UUID,
    status import_indexer_result_status,
    detail VARCHAR,
    created_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT * FROM import_job_list_results_v1(import_job_public_id_input);
END;
$$;
