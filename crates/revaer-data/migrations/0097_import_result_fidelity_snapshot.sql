-- Preserve imported configuration fidelity snapshots for import results.

ALTER TABLE import_indexer_result
    ADD COLUMN IF NOT EXISTS resolved_is_enabled BOOLEAN,
    ADD COLUMN IF NOT EXISTS resolved_priority INTEGER
        CHECK (
            resolved_priority IS NULL
            OR resolved_priority BETWEEN 0 AND 100
        ),
    ADD COLUMN IF NOT EXISTS missing_secret_fields INTEGER NOT NULL DEFAULT 0
        CHECK (missing_secret_fields >= 0);

CREATE TABLE IF NOT EXISTS import_indexer_result_media_domain (
    import_indexer_result_media_domain_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    import_indexer_result_id BIGINT NOT NULL
        REFERENCES import_indexer_result (import_indexer_result_id)
        ON DELETE CASCADE,
    media_domain_id BIGINT NOT NULL
        REFERENCES media_domain (media_domain_id),
    CONSTRAINT import_indexer_result_media_domain_uq UNIQUE (
        import_indexer_result_id,
        media_domain_id
    )
);

CREATE TABLE IF NOT EXISTS import_indexer_result_tag (
    import_indexer_result_tag_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    import_indexer_result_id BIGINT NOT NULL
        REFERENCES import_indexer_result (import_indexer_result_id)
        ON DELETE CASCADE,
    tag_id BIGINT NOT NULL
        REFERENCES tag (tag_id),
    CONSTRAINT import_indexer_result_tag_uq UNIQUE (
        import_indexer_result_id,
        tag_id
    )
);

DROP FUNCTION IF EXISTS import_job_list_results(UUID);
DROP FUNCTION IF EXISTS import_job_list_results_v1(UUID);

CREATE OR REPLACE FUNCTION import_job_list_results_v1(
    import_job_public_id_input UUID
)
RETURNS TABLE(
    prowlarr_identifier VARCHAR,
    upstream_slug VARCHAR,
    indexer_instance_public_id UUID,
    status import_indexer_result_status,
    detail VARCHAR,
    resolved_is_enabled BOOLEAN,
    resolved_priority INTEGER,
    missing_secret_fields INTEGER,
    media_domain_keys VARCHAR[],
    tag_keys VARCHAR[],
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
           r.resolved_is_enabled,
           r.resolved_priority,
           r.missing_secret_fields,
           ARRAY(
               SELECT md.media_domain_key::VARCHAR
               FROM import_indexer_result_media_domain ird
               JOIN media_domain md
                 ON md.media_domain_id = ird.media_domain_id
               WHERE ird.import_indexer_result_id = r.import_indexer_result_id
               ORDER BY md.media_domain_key
           ),
           ARRAY(
               SELECT t.tag_key
               FROM import_indexer_result_tag irt
               JOIN tag t
                 ON t.tag_id = irt.tag_id
               WHERE irt.import_indexer_result_id = r.import_indexer_result_id
               ORDER BY t.tag_key
           ),
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
    resolved_is_enabled BOOLEAN,
    resolved_priority INTEGER,
    missing_secret_fields INTEGER,
    media_domain_keys VARCHAR[],
    tag_keys VARCHAR[],
    created_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT * FROM import_job_list_results_v1(import_job_public_id_input);
END;
$$;
