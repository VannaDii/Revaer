-- Enforce FK on-delete rules for indexer instance children.

ALTER TABLE indexer_instance_media_domain
    DROP CONSTRAINT IF EXISTS indexer_instance_media_domain_indexer_instance_id_fkey;
ALTER TABLE indexer_instance_media_domain
    ADD CONSTRAINT indexer_instance_media_domain_indexer_instance_id_fkey
        FOREIGN KEY (indexer_instance_id)
        REFERENCES indexer_instance (indexer_instance_id) ON DELETE CASCADE;

ALTER TABLE indexer_instance_tag
    DROP CONSTRAINT IF EXISTS indexer_instance_tag_indexer_instance_id_fkey;
ALTER TABLE indexer_instance_tag
    ADD CONSTRAINT indexer_instance_tag_indexer_instance_id_fkey
        FOREIGN KEY (indexer_instance_id)
        REFERENCES indexer_instance (indexer_instance_id) ON DELETE CASCADE;

ALTER TABLE indexer_rss_subscription
    DROP CONSTRAINT IF EXISTS indexer_rss_subscription_indexer_instance_id_fkey;
ALTER TABLE indexer_rss_subscription
    ADD CONSTRAINT indexer_rss_subscription_indexer_instance_id_fkey
        FOREIGN KEY (indexer_instance_id)
        REFERENCES indexer_instance (indexer_instance_id) ON DELETE CASCADE;

ALTER TABLE indexer_rss_item_seen
    DROP CONSTRAINT IF EXISTS indexer_rss_item_seen_indexer_instance_id_fkey;
ALTER TABLE indexer_rss_item_seen
    ADD CONSTRAINT indexer_rss_item_seen_indexer_instance_id_fkey
        FOREIGN KEY (indexer_instance_id)
        REFERENCES indexer_instance (indexer_instance_id) ON DELETE CASCADE;

ALTER TABLE indexer_instance_field_value
    DROP CONSTRAINT IF EXISTS indexer_instance_field_value_indexer_instance_id_fkey;
ALTER TABLE indexer_instance_field_value
    ADD CONSTRAINT indexer_instance_field_value_indexer_instance_id_fkey
        FOREIGN KEY (indexer_instance_id)
        REFERENCES indexer_instance (indexer_instance_id) ON DELETE CASCADE;

ALTER TABLE indexer_instance_import_blob
    DROP CONSTRAINT IF EXISTS indexer_instance_import_blob_indexer_instance_id_fkey;
ALTER TABLE indexer_instance_import_blob
    ADD CONSTRAINT indexer_instance_import_blob_indexer_instance_id_fkey
        FOREIGN KEY (indexer_instance_id)
        REFERENCES indexer_instance (indexer_instance_id) ON DELETE CASCADE;

ALTER TABLE indexer_connectivity_profile
    DROP CONSTRAINT IF EXISTS indexer_connectivity_profile_indexer_instance_id_fkey;
ALTER TABLE indexer_connectivity_profile
    ADD CONSTRAINT indexer_connectivity_profile_indexer_instance_id_fkey
        FOREIGN KEY (indexer_instance_id)
        REFERENCES indexer_instance (indexer_instance_id) ON DELETE CASCADE;

ALTER TABLE indexer_health_event
    DROP CONSTRAINT IF EXISTS indexer_health_event_indexer_instance_id_fkey;
ALTER TABLE indexer_health_event
    ADD CONSTRAINT indexer_health_event_indexer_instance_id_fkey
        FOREIGN KEY (indexer_instance_id)
        REFERENCES indexer_instance (indexer_instance_id) ON DELETE CASCADE;
