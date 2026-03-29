-- Add FK for policy_set.created_for_search_request_id per ERD.

ALTER TABLE policy_set
    DROP CONSTRAINT IF EXISTS policy_set_created_for_search_request_id_fkey;
ALTER TABLE policy_set
    ADD CONSTRAINT policy_set_created_for_search_request_id_fkey
        FOREIGN KEY (created_for_search_request_id)
        REFERENCES search_request (search_request_id) ON DELETE CASCADE;
