BEGIN;

CREATE OR REPLACE FUNCTION revaer_bump_revision()
RETURNS TRIGGER AS $$
DECLARE
    revision_setting TEXT;
    effective_revision BIGINT;
BEGIN
    BEGIN
        revision_setting := current_setting('revaer.current_revision', true);
    EXCEPTION
        WHEN others THEN
            revision_setting := NULL;
    END;

    IF revision_setting IS NULL OR revision_setting = '' THEN
        UPDATE settings_revision
        SET revision = revision + 1,
            updated_at = now()
        WHERE id = 1
        RETURNING revision INTO effective_revision;

        PERFORM set_config('revaer.current_revision', effective_revision::TEXT, true);
    ELSE
        effective_revision := revision_setting::BIGINT;
    END IF;

    PERFORM pg_notify(
        'revaer_settings_changed',
        format('%s:%s:%s', TG_TABLE_NAME, effective_revision, TG_OP)
    );

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    ELSE
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;

COMMIT;
