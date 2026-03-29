-- Align policy_action -> decision_type conversion for search filter decision logging.
DROP CAST IF EXISTS (policy_action AS decision_type);

CREATE OR REPLACE FUNCTION policy_action_to_decision_type(action_input policy_action)
RETURNS decision_type
LANGUAGE SQL
IMMUTABLE
AS $$
    SELECT CASE action_input
        WHEN 'drop_canonical'::policy_action THEN 'drop_canonical'::decision_type
        WHEN 'drop_source'::policy_action THEN 'drop_source'::decision_type
        WHEN 'downrank'::policy_action THEN 'downrank'::decision_type
        WHEN 'flag'::policy_action THEN 'flag'::decision_type
        WHEN 'require'::policy_action THEN 'flag'::decision_type
        WHEN 'prefer'::policy_action THEN 'flag'::decision_type
    END;
$$;

CREATE CAST (policy_action AS decision_type)
    WITH FUNCTION policy_action_to_decision_type(policy_action)
    AS ASSIGNMENT;
