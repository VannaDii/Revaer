-- Seal streaming pages and finalize search requests when all runs are terminal.

CREATE OR REPLACE FUNCTION search_request_finalize_on_runs_terminal_v1()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    request_status_value search_status;
    finalized_at_value TIMESTAMPTZ;
BEGIN
    IF NEW.status NOT IN ('finished', 'failed', 'canceled') THEN
        RETURN NEW;
    END IF;

    IF TG_OP = 'UPDATE' AND OLD.status = NEW.status THEN
        RETURN NEW;
    END IF;

    SELECT status
    INTO request_status_value
    FROM search_request
    WHERE search_request_id = NEW.search_request_id
    FOR UPDATE;

    IF request_status_value IS NULL OR request_status_value <> 'running' THEN
        RETURN NEW;
    END IF;

    IF EXISTS (
        SELECT 1
        FROM search_request_indexer_run
        WHERE search_request_id = NEW.search_request_id
          AND status IN ('queued', 'running')
    ) THEN
        RETURN NEW;
    END IF;

    finalized_at_value := now();

    UPDATE search_request
    SET status = 'finished',
        finished_at = finalized_at_value
    WHERE search_request_id = NEW.search_request_id
      AND status = 'running';

    IF FOUND THEN
        UPDATE search_page
        SET sealed_at = finalized_at_value
        WHERE search_request_id = NEW.search_request_id
          AND sealed_at IS NULL;
    END IF;

    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS search_request_finalize_on_runs_terminal_trigger
ON search_request_indexer_run;

CREATE TRIGGER search_request_finalize_on_runs_terminal_trigger
AFTER INSERT OR UPDATE OF status ON search_request_indexer_run
FOR EACH ROW
EXECUTE FUNCTION search_request_finalize_on_runs_terminal_v1();
