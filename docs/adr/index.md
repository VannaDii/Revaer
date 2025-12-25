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
- [029](029-torrent-authoring-labels-and-metadata.md) – Torrent authoring, labels, and metadata updates
- [030](030-migration-consolidation.md) – Migration consolidation for initial setup
- [031](031-ui-asset-sync.md) – UI Nexus asset sync tooling
- [032](032-torrent-ffi-audit-closeout.md) – Torrent FFI audit closeout
- [033](033-ui-sse-auth-setup.md) – UI SSE + auth/setup wiring
- [034](034-ui-sse-store-apiclient.md) – UI SSE normalization and ApiClient singleton
- [035](035-advisory-rustsec-2021-0065.md) – Advisory RUSTSEC-2021-0065 temporary ignore
- [036](036-asset-sync-test-stability.md) – Asset sync test stability under parallel runs
- [037](037-ui-row-slices-system-rates.md) – UI row slices and system-rate store wiring
- [038](038-ui-api-models-filters-paging.md) – UI shared API models and torrent query paging state
- [039](039-ui-store-api-rate-limit.md) – UI store, API coverage, and rate-limit retries
- [040](040-ui-labels-policy.md) – UI label policy editor and API wiring
- [041](041-ui-health-shortcuts.md) – UI health view and label shortcuts
- [042](042-ui-metrics-copy.md) – UI metrics copy button
- [043](043-ui-settings-bypass-auth.md) – UI settings bypass local auth toggle
- [044](044-ui-api-client-options-selection.md) – UI ApiClient torrent options/selection endpoints
- [045](045-ui-icon-system.md) – UI icon components and icon button standardization
- [046](046-ui-torrent-filters-pagination.md) – UI torrent filters, pagination, and URL sync
- [047](047-ui-torrent-updated-column.md) – UI torrent list updated timestamp column
- [048](048-ui-torrent-actions-bulk-controls.md) – UI torrent row actions, bulk controls, and rate/remove dialogs
- [049](049-ui-detail-overview-files-options.md) – UI detail drawer overview/files/options
- [050](050-ui-torrent-fab-create-modals.md) – UI torrent FAB, add modal, and create-torrent authoring flow
- [051](051-ui-api-models-primitives.md) – UI shared API models and UX primitives
- [052](052-ui-nexus-dashboard.md) – UI dashboard migration to Nexus vendor layout

## Suggested Workflow
1. Create a new ADR using the template in `docs/adr/template.md`.
2. Give it a sequential identifier (e.g., `001`, `002`) and a concise title.
3. Capture context, decision, consequences, and follow-up actions.
4. Reference ADRs from code comments or docs where the decision applies.
