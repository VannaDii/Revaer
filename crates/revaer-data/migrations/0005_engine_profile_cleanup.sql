-- Remove legacy per-field engine profile updaters in favour of the unified
-- `update_engine_profile` procedure to keep DB/API validation in lockstep.

DROP FUNCTION IF EXISTS revaer_config.update_engine_implementation(UUID, TEXT);
DROP FUNCTION IF EXISTS revaer_config.update_engine_listen_port(UUID, INTEGER);
DROP FUNCTION IF EXISTS revaer_config.update_engine_boolean_field(UUID, TEXT, BOOLEAN);
DROP FUNCTION IF EXISTS revaer_config.update_engine_encryption(UUID, TEXT);
DROP FUNCTION IF EXISTS revaer_config.update_engine_max_active(UUID, INTEGER);
DROP FUNCTION IF EXISTS revaer_config.update_engine_rate_field(UUID, TEXT, BIGINT);
DROP FUNCTION IF EXISTS revaer_config.update_engine_text_field(UUID, TEXT, TEXT);
DROP FUNCTION IF EXISTS revaer_config.update_engine_tracker(UUID, JSONB);
