# Architecture Decision Records

ADR documents capture the rationale behind significant technical decisions.

## Catalogue
- [001](001-configuration-revisioning.md) – Configuration revisioning
- [002](002-setup-token-lifecycle.md) – Setup token lifecycle
- [003](003-libtorrent-session-runner.md) – Libtorrent session runner
- [004](004-phase-one-delivery.md) – Phase one delivery
- [005](005-fsops-pipeline.md) – FS operations pipeline
- [006](006-api-cli-contract.md) – API/CLI contract
- [007](007-security-posture.md) – Security posture
- [008](008-phase-one-remaining-task.md) – Remaining phase-one tasks
- [009](009-fsops-permission-hardening.md) – FS ops permission hardening
- [010](010-agent-compliance-sweep.md) – Agent compliance sweep
- [011](011-coverage-hardening-phase-two.md) – Coverage hardening
- [012](012-agent-compliance-refresh.md) – Agent compliance refresh
- [013](013-runtime-persistence.md) – Runtime persistence
- [014](014-data-access-layer.md) – Data access layer
- [015](015-agent-compliance-hardening.md) – Agent compliance hardening
- [016](016-libtorrent-restoration.md) – Libtorrent restoration
- [017](017-sqlx-named-bind.md) – Avoid `sqlx-named-bind`
- [018](018-retire-testcontainers.md) – Retire testcontainers
- [019](019-advisory-rustsec-2024-0370.md) – Advisory RUSTSEC-2024-0370 temporary ignore
- [020](020-torrent-engine-precursors.md) – Torrent engine precursor hardening
- [021](021-torrent-precursor-enforcement.md) – Torrent precursor enforcement
- [022](022-torrent-settings-parity.md) – Torrent settings parity and observability
- [023](023-tracker-config-wiring.md) – Tracker config wiring and persistence
- [024](024-seeding-stop-criteria.md) – Seeding stop criteria and overrides
- [025](025-seed-mode-add-as-complete.md) – Seed mode admission with optional hash sampling
- [026](026-queue-auto-managed-and-pex.md) – Queue auto-managed defaults and PEX threading
- [027](027-choking-and-super-seeding.md) – Choking strategy and super-seeding configuration
- [028](028-qbittorrent-parity-and-tracker-tls.md) – qBittorrent parity and tracker TLS wiring

## Suggested Workflow
1. Create a new ADR using the template in `docs/adr/template.md`.
2. Give it a sequential identifier (e.g., `001`, `002`) and a concise title.
3. Capture context, decision, consequences, and follow-up actions.
4. Reference ADRs from code comments or docs where the decision applies.
