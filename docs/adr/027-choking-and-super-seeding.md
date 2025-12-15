# Choking Strategy And Super-Seeding Configuration

- Status: Accepted
- Date: 2025-12-14
- Context:
  - TORRENT_GAPS requires configurable choke/unchoke strategy and super-seeding defaults.
  - We must keep config/runtime/FFI/native paths aligned while preserving safe defaults.
  - API and persistence need to surface new knobs without regressing existing behaviour.
- Decision:
  - Added engine profile fields for choking (`choking_algorithm`, `seed_choking_algorithm`, `strict_super_seeding`, `optimistic_unchoke_slots`, `max_queued_disk_bytes`) and `super_seeding`.
  - Normalise/validate values with guard-rail warnings; persist via a single stored-proc update path and migration `0006_choking_and_super_seeding.sql`.
  - Thread new options through runtime config, FFI structs, and native session (`settings_pack` + per-torrent flags). Per-torrent `super_seeding` overrides are stored with metadata.
  - Updated API models/OpenAPI and added tests covering canonicalisation, clamping, and FFI planning.
- Consequences:
  - Engine config now exposes advanced choke/seeding controls; defaults remain safe (`fixed_slots`, `round_robin`, super-seeding off).
  - Metadata format and DB schema gain new fields; migration is required before runtime use.
  - Native session applies and can reset choking settings; add-path respects per-torrent super-seeding.
- Follow-up:
  - Expand native coverage for strict super-seeding and queue byte limits when integration harness is available.
  - Monitor telemetry for churn when users toggle new fields; add UX help text where appropriate.
