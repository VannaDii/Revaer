# Indexer hash identity rules verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - The ERD defines hash identity rules for infohash v1/v2, magnet hashes, title
    normalization, and title-size fallback hashing.
  - We need to confirm the schema and ingest procedures enforce these rules.
- Decision:
  - Verified canonical tables enforce hash shapes and lowercase normalization:
    `canonical_torrent` and `canonical_torrent_source` validate infohash and
    magnet hashes, plus enforce lowercase `title_normalized`.
  - Verified ingest procedures implement ERD hash derivations:
    `normalize_title_v1`, `derive_magnet_hash_v1`, and
    `compute_title_size_hash_v1` in `indexer_search_result_ingest_proc.sql`
    implement normalization, magnet hash derivation, and title-size hashing.
  - Verified identity strategy selection uses infohash v2, infohash v1, magnet
    hash, or title-size fallback per ERD.
- Consequences:
  - Hash identity rules are enforced consistently at the DB layer and in ingest
    logic.
  - Canonicalization can reliably deduplicate sources without depending on
    caller behavior.
- Follow-up:
  - Re-verify if hash derivation logic changes or new identity strategies are
    added.

## Task record

- Motivation:
  - Confirm ERD hash identity rules are implemented in schema and procedures.
- Design notes:
  - Reviewed `0022_indexer_canonicalization.sql` for hash constraints and
    identity strategy checks.
  - Reviewed `0052_indexer_search_result_ingest_proc.sql` for normalization and
    hash derivation functions.
- Test coverage summary:
  - Documentation-only verification; existing ingest tests cover hashing paths.
- Observability updates:
  - None.
- Risk & rollback plan:
  - Risk: regressions in hash derivation cause identity splits. Roll back by
    reverting procedure changes and revalidating constraints.
- Dependency rationale:
  - No new dependencies added.
