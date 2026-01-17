# ADRs

## Suggested Use Workflow

1. Create a new ADR using the template in `docs/adr/template.md`.
2. Give it a sequential identifier (e.g., `001`, `002`) and a concise title.
3. Capture context, decision, consequences, and follow-up actions.
4. Append the new ADR entry to the end of the Catalogue list above.
5. Append the same entry under `ADRs` in `docs/SUMMARY.md`, keeping it nested so the sidebar stays collapsed.
6. Reference ADRs from code comments or docs where the decision applies.

## Catalogue

-   [Template](template.md) – ADR template
-   [001](001-configuration-revisioning.md) – Configuration revisioning
-   [002](002-setup-token-lifecycle.md) – Setup token lifecycle
-   [003](003-libtorrent-session-runner.md) – Libtorrent session runner
-   [004](004-phase-one-delivery.md) – Phase one delivery
-   [005](005-fsops-pipeline.md) – FS operations pipeline
-   [006](006-api-cli-contract.md) – API/CLI contract
-   [007](007-security-posture.md) – Security posture
-   [008](008-phase-one-remaining-task.md) – Remaining phase-one tasks
-   [009](009-fsops-permission-hardening.md) – FS ops permission hardening
-   [010](010-agent-compliance-sweep.md) – Agent compliance sweep
-   [011](011-coverage-hardening-phase-two.md) – Coverage hardening
-   [012](012-agent-compliance-refresh.md) – Agent compliance refresh
-   [013](013-runtime-persistence.md) – Runtime persistence
-   [014](014-data-access-layer.md) – Data access layer
-   [015](015-agent-compliance-hardening.md) – Agent compliance hardening
-   [016](016-libtorrent-restoration.md) – Libtorrent restoration
-   [017](017-sqlx-named-bind.md) – Avoid `sqlx-named-bind`
-   [018](018-retire-testcontainers.md) – Retire testcontainers
-   [019](019-advisory-rustsec-2024-0370.md) – Advisory RUSTSEC-2024-0370 temporary ignore
-   [020](020-torrent-engine-precursors.md) – Torrent engine precursor hardening
-   [021](021-torrent-precursor-enforcement.md) – Torrent precursor enforcement
-   [022](022-torrent-settings-parity.md) – Torrent settings parity and observability
-   [023](023-tracker-config-wiring.md) – Tracker config wiring and persistence
-   [024](024-seeding-stop-criteria.md) – Seeding stop criteria and overrides
-   [025](025-seed-mode-add-as-complete.md) – Seed mode admission with optional hash sampling
-   [026](026-queue-auto-managed-and-pex.md) – Queue auto-managed defaults and PEX threading
-   [027](027-choking-and-super-seeding.md) – Choking strategy and super-seeding configuration
-   [028](028-qbittorrent-parity-and-tracker-tls.md) – qBittorrent parity and tracker TLS wiring
-   [029](029-torrent-authoring-labels-and-metadata.md) – Torrent authoring, labels, and metadata updates
-   [030](030-migration-consolidation.md) – Migration consolidation for initial setup
-   [031](031-ui-asset-sync.md) – UI Nexus asset sync tooling
-   [032](032-torrent-ffi-audit-closeout.md) – Torrent FFI audit closeout
-   [033](033-ui-sse-auth-setup.md) – UI SSE + auth/setup wiring
-   [034](034-ui-sse-store-apiclient.md) – UI SSE normalization and ApiClient singleton
-   [035](035-advisory-rustsec-2021-0065.md) – Advisory RUSTSEC-2021-0065 temporary ignore
-   [036](036-asset-sync-test-stability.md) – Asset sync test stability under parallel runs
-   [037](037-ui-row-slices-system-rates.md) – UI row slices and system-rate store wiring
-   [038](038-ui-api-models-filters-paging.md) – UI shared API models and torrent query paging state
-   [039](039-ui-store-api-rate-limit.md) – UI store, API coverage, and rate-limit retries
-   [040](040-ui-labels-policy.md) – UI label policy editor and API wiring
-   [041](041-ui-health-shortcuts.md) – UI health view and label shortcuts
-   [042](042-ui-metrics-copy.md) – UI metrics copy button
-   [043](043-ui-settings-bypass-auth.md) – UI settings bypass local auth toggle
-   [044](044-ui-api-client-options-selection.md) – UI ApiClient torrent options/selection endpoints
-   [045](045-ui-icon-system.md) – UI icon components and icon button standardization
-   [046](046-ui-torrent-filters-pagination.md) – UI torrent filters, pagination, and URL sync
-   [047](047-ui-torrent-updated-column.md) – UI torrent list updated timestamp column
-   [048](048-ui-torrent-actions-bulk-controls.md) – UI torrent row actions, bulk controls, and rate/remove dialogs
-   [049](049-ui-detail-overview-files-options.md) – UI detail drawer overview/files/options
-   [050](050-ui-torrent-fab-create-modals.md) – UI torrent FAB, add modal, and create-torrent authoring flow
-   [051](051-ui-api-models-primitives.md) – UI shared API models and UX primitives
-   [052](052-ui-nexus-dashboard.md) – UI dashboard migration to Nexus vendor layout
-   [053](053-ui-dashboard-hardline-rebuild.md) – UI dashboard hardline rebuild
-   [054](054-ui-dashboard-nexus-parity.md) – UI dashboard Nexus parity tweaks
-   [055](055-factory-reset-bootstrap-api-key.md) – Factory reset and bootstrap API key
-   [056](056-factory-reset-bootstrap-auth-fallback.md) – Factory reset auth fallback when no API keys exist
-   [057](057-ui-settings-tabs-controls.md) – UI settings tabs and editor controls
-   [058](058-settings-logs-fs-browser.md) – UI settings controls, logs stream, and filesystem browser
-   [059](059-migration-rebaseline.md) – Migration rebaseline and JSON backfill guardrails
-   [060](060-auth-expiry-error-context.md) – Auth expiry enforcement and structured error context
-   [061](061-api-i18n-openapi-assets.md) – API error i18n and OpenAPI asset constants
-   [062](062-eventbus-publish-guardrails.md) – Event bus publish guardrails and API i18n cleanup
-   [063](063-ci-compliance-cleanup.md) – CI compliance cleanup for test error handling
-   [064](064-factory-reset-error-context.md) – Factory reset error context and allow-path validation
-   [065](065-auth-mode-refresh.md) – API key refresh and no-auth setup mode
-   [066](066-factory-reset-sse-setup.md) – Factory reset UX fallback and SSE setup gating
-   [067](067-logs-ansi-rendering.md) – Logs ANSI rendering and bounded buffer
-   [068](068-agent-compliance-clippy-cargo.md) – Agent compliance clippy cargo linting
-   [069](069-docs-mdbook-mermaid-version.md) – Pin mdbook-mermaid for docs builds
-   [070](070-dashboard-ui-checklist.md) – Dashboard UI checklist completion and auth/SSE hardening
-   [071](071-libtorrent-native-fallback.md) – Libtorrent native fallback for default CI
-   [072](072-agent-compliance-refactor.md) – Agent compliance refactor (UI + HTTP + Config Layout)
-   [073](073-ui-checklist-followups.md) – UI checklist follow-ups: SSE detail refresh, labels shortcuts, strict i18n, and anymap removal
-   [074](074-vendored-yewdux-latest-yew.md) – Temporary vendoring of yewdux for latest Yew compatibility
-   [075](075-coverage-gate-tests.md) – Coverage gate tests for config loader and data toggles
-   [076](076-hashbrown-multiple-versions-exception.md) – Temporary clippy exception for hashbrown multiple versions
-   [077](077-ui-menu-interactions.md) – UI menu interactions
-   [078](078-local-auth-bypass-guardrails.md) – Local auth bypass guardrails
-   [079](079-advisory-rustsec-2025-0141.md) – Advisory RUSTSEC-2025-0141 temporary ignore
-   [080](080-local-auth-bypass-reliability.md) – Local auth bypass reliability
-   [081](081-playwright-e2e-suite.md) – Playwright E2E test suite
-   [082](082-e2e-gate-and-selectors.md) – E2E gate and selector stability
-   [083](083-api-preflight-e2e.md) – API preflight before UI E2E
-   [084](084-e2e-api-coverage-temp-db.md) – E2E API coverage with temp databases
-   [085](085-e2e-openapi-client-and-coverage.md) – E2E OpenAPI client and unified coverage
