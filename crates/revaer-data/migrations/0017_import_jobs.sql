-- Import jobs and results.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'import_source') THEN
        CREATE TYPE import_source AS ENUM ('prowlarr_api', 'prowlarr_backup');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'import_job_status') THEN
        CREATE TYPE import_job_status AS ENUM (
            'pending',
            'running',
            'completed',
            'failed',
            'canceled'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'import_indexer_result_status') THEN
        CREATE TYPE import_indexer_result_status AS ENUM (
            'imported_ready',
            'imported_needs_secret',
            'imported_test_failed',
            'unmapped_definition',
            'skipped_duplicate'
        );
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS import_job (
    import_job_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    import_job_public_id UUID NOT NULL,
    target_search_profile_id BIGINT
        REFERENCES search_profile (search_profile_id),
    target_torznab_instance_id BIGINT
        REFERENCES torznab_instance (torznab_instance_id),
    created_by_user_id BIGINT NOT NULL
        REFERENCES app_user (user_id),
    source import_source NOT NULL,
    is_dry_run BOOLEAN NOT NULL DEFAULT FALSE,
    status import_job_status NOT NULL,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    error_detail VARCHAR(1024),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT import_job_public_id_uq UNIQUE (import_job_public_id)
);

CREATE TABLE IF NOT EXISTS import_indexer_result (
    import_indexer_result_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    import_job_id BIGINT NOT NULL
        REFERENCES import_job (import_job_id),
    prowlarr_identifier VARCHAR(256) NOT NULL,
    upstream_slug VARCHAR(128),
    indexer_instance_id BIGINT,
    status import_indexer_result_status NOT NULL,
    detail VARCHAR(512),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT import_indexer_result_uq UNIQUE (
        import_job_id,
        prowlarr_identifier
    )
);
