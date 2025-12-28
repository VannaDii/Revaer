-- BEGIN 0002_factory_reset.sql
-- Factory reset procedure for rebuilding the database to defaults.

CREATE OR REPLACE FUNCTION revaer_config.factory_reset()
RETURNS VOID AS
$$
DECLARE
    rec RECORD;
BEGIN
    FOR rec IN
        SELECT schemaname, tablename
        FROM pg_tables
        WHERE schemaname IN ('public', 'revaer_runtime')
          AND tablename <> '_sqlx_migrations'
    LOOP
        EXECUTE format(
            'TRUNCATE TABLE %I.%I RESTART IDENTITY CASCADE',
            rec.schemaname,
            rec.tablename
        );
    END LOOP;

    INSERT INTO public.settings_revision (id, revision)
    VALUES (1, 0)
    ON CONFLICT (id) DO UPDATE
    SET revision = EXCLUDED.revision,
        updated_at = now();

    INSERT INTO public.app_profile (id, mode, instance_name)
    VALUES (
        '00000000-0000-0000-0000-000000000001',
        'setup',
        'revaer'
    );

    INSERT INTO public.engine_profile (id, implementation, resume_dir, download_root)
    VALUES (
        '00000000-0000-0000-0000-000000000002',
        'libtorrent',
        '/var/lib/revaer/state',
        '/data/staging'
    );

    INSERT INTO public.fs_policy (id, library_root, allow_paths)
    VALUES (
        '00000000-0000-0000-0000-000000000003',
        '/data/library',
        '["/data/staging", "/data/library"]'::jsonb
    );
END;
$$ LANGUAGE plpgsql;

-- END 0002_factory_reset.sql
