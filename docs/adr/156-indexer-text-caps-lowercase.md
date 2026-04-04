# Indexer text caps and lowercase key enforcement verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - The ERD mandates text column caps and lowercase enforcement for key/slug
    fields (varchar(128) keys, varchar(256) names, varchar(2048) URLs,
    varchar(512) regex/text patterns, varchar(1024) notes).
  - We need to confirm the schema enforces these caps and lowercase checks before
    expanding APIs and UI validation.
- Decision:
  - Verified key/slug fields use `VARCHAR(128)` with lowercase CHECKs where
    required (e.g., `tag.tag_key`, `indexer_definition.upstream_slug`,
    `indexer_definition_field.name`).
  - Verified display names are capped at `VARCHAR(256)` across core catalog
    tables (e.g., `tag.display_name`, `indexer_definition.display_name`,
    `search_profile.display_name`, `policy_set.display_name`).
  - Verified URL fields use `VARCHAR(2048)` (e.g., `search_request_source_observation`
    `details_url`, `download_url`, `magnet_uri`).
  - Verified regex/pattern text caps at `VARCHAR(512)` and notes/detail caps at
    `VARCHAR(1024)` (e.g., `indexer_definition_field_validation.text_value`,
    `search_request.query_text`, `search_request.error_detail`,
    `policy_rule.rationale`).
- Consequences:
  - Schema enforces ERD text caps and lowercase rules, preventing oversized or
    improperly cased keys from entering the database.
  - API validation can align with these constraints without risking truncation.
- Follow-up:
  - Re-verify any new text columns added to the indexer schema.

## Task record

- Motivation:
  - Confirm text caps and lowercase key enforcement align with ERD rules.
- Design notes:
  - Reviewed `0012_indexer_core.sql` (`tag_key` lower-case CHECK, display name sizes).
  - Reviewed `0013_indexer_definitions.sql` (slug/name lowercase CHECKs and
    text caps).
  - Reviewed `0023_indexer_search_requests.sql` for URL/text/detail caps.
  - Reviewed `0019_policy_sets.sql` for rationale/text caps.
- Test coverage summary:
  - Documentation-only verification; no new tests added.
- Observability updates:
  - None.
- Risk & rollback plan:
  - Risk: new columns exceed caps or miss lowercase checks. Roll back by
    adjusting migrations and re-validating constraints.
- Dependency rationale:
  - No new dependencies added.
