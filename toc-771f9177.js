// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="index.html">Overview</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="phase-one-roadmap.html">Phase One Roadmap</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="phase-one-remaining-spec.html">Phase One Remaining Spec</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="runbook.html">Runbook</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="release-checklist.html">Release Checklist</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="ui/index.html">Web UI - Phase 1</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="ui/flows.html">Web UI Flows</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="platform/configuration.html">Configuration Surface</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="platform/api.html">HTTP API</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="platform/cli.html">CLI Reference</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="platform/torrent-flows.html">Torrent Flows</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="platform/native-tests.html">Native Libtorrent Tests</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="api/index.html">API Overview</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="api/openapi.html">OpenAPI Reference</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="api/openapi-gaps.html">OpenAPI Gaps</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="api/guides/indexer-migration-rollback.html">Indexer Migration Rollback</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/index.html">ADRs</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/template.html">ADR Template</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/001-configuration-revisioning.html">001: Configuration revisioning</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/002-setup-token-lifecycle.html">002: Setup token lifecycle</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/003-libtorrent-session-runner.html">003: Libtorrent session runner</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/004-phase-one-delivery.html">004: Phase one delivery</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/005-fsops-pipeline.html">005: FS operations pipeline</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/006-api-cli-contract.html">006: API/CLI contract</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/007-security-posture.html">007: Security posture</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/008-phase-one-remaining-task.html">008: Remaining phase-one tasks</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/009-fsops-permission-hardening.html">009: FS ops permission hardening</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/010-agent-compliance-sweep.html">010: Agent compliance sweep</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/011-coverage-hardening-phase-two.html">011: Coverage hardening</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/012-agent-compliance-refresh.html">012: Agent compliance refresh</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/013-runtime-persistence.html">013: Runtime persistence</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/014-data-access-layer.html">014: Data access layer</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/015-agent-compliance-hardening.html">015: Agent compliance hardening</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/016-libtorrent-restoration.html">016: Libtorrent restoration</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/017-sqlx-named-bind.html">017: Avoid sqlx-named-bind</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/018-retire-testcontainers.html">018: Retire testcontainers</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/019-advisory-rustsec-2024-0370.html">019: Advisory RUSTSEC-2024-0370 temporary ignore</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/020-torrent-engine-precursors.html">020: Torrent engine precursor hardening</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/021-torrent-precursor-enforcement.html">021: Torrent precursor enforcement</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/022-torrent-settings-parity.html">022: Torrent settings parity and observability</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/023-tracker-config-wiring.html">023: Tracker config wiring and persistence</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/024-seeding-stop-criteria.html">024: Seeding stop criteria and overrides</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/025-seed-mode-add-as-complete.html">025: Seed mode admission with optional hash sampling</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/026-queue-auto-managed-and-pex.html">026: Queue auto-managed defaults and PEX threading</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/027-choking-and-super-seeding.html">027: Choking strategy and super-seeding configuration</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/028-qbittorrent-parity-and-tracker-tls.html">028: qBittorrent parity and tracker TLS wiring</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/029-torrent-authoring-labels-and-metadata.html">029: Torrent authoring, labels, and metadata updates</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/030-migration-consolidation.html">030: Migration consolidation for initial setup</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/031-ui-asset-sync.html">031: UI Nexus asset sync tooling</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/032-torrent-ffi-audit-closeout.html">032: Torrent FFI audit closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/033-ui-sse-auth-setup.html">033: UI SSE + auth/setup wiring</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/034-ui-sse-store-apiclient.html">034: UI SSE normalization and ApiClient singleton</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/035-advisory-rustsec-2021-0065.html">035: Advisory RUSTSEC-2021-0065 temporary ignore</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/036-asset-sync-test-stability.html">036: Asset sync test stability under parallel runs</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/037-ui-row-slices-system-rates.html">037: UI row slices and system-rate store wiring</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/038-ui-api-models-filters-paging.html">038: UI shared API models and torrent query paging state</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/039-ui-store-api-rate-limit.html">039: UI store, API coverage, and rate-limit retries</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/040-ui-labels-policy.html">040: UI label policy editor and API wiring</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/041-ui-health-shortcuts.html">041: UI health view and label shortcuts</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/042-ui-metrics-copy.html">042: UI metrics copy button</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/043-ui-settings-bypass-auth.html">043: UI settings bypass local auth toggle</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/044-ui-api-client-options-selection.html">044: UI ApiClient torrent options/selection endpoints</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/045-ui-icon-system.html">045: UI icon components and icon button standardization</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/046-ui-torrent-filters-pagination.html">046: UI torrent filters, pagination, and URL sync</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/047-ui-torrent-updated-column.html">047: UI torrent list updated timestamp column</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/048-ui-torrent-actions-bulk-controls.html">048: UI torrent row actions, bulk controls, and rate/remove dialogs</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/049-ui-detail-overview-files-options.html">049: UI detail drawer overview/files/options</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/050-ui-torrent-fab-create-modals.html">050: UI torrent FAB, add modal, and create-torrent authoring flow</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/051-ui-api-models-primitives.html">051: UI shared API models and UX primitives</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/052-ui-nexus-dashboard.html">052: UI dashboard migration to Nexus vendor layout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/053-ui-dashboard-hardline-rebuild.html">053: UI dashboard hardline rebuild</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/054-ui-dashboard-nexus-parity.html">054: UI dashboard Nexus parity tweaks</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/055-factory-reset-bootstrap-api-key.html">055: Factory reset and bootstrap API key</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/056-factory-reset-bootstrap-auth-fallback.html">056: Factory reset auth fallback when no API keys exist</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/057-ui-settings-tabs-controls.html">057: UI settings tabs and editor controls</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/058-settings-logs-fs-browser.html">058: UI settings controls, logs stream, and filesystem browser</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/059-migration-rebaseline.html">059: Migration rebaseline and JSON backfill guardrails</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/060-auth-expiry-error-context.html">060: Auth expiry enforcement and structured error context</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/061-api-i18n-openapi-assets.html">061: API error i18n and OpenAPI asset constants</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/062-eventbus-publish-guardrails.html">062: Event bus publish guardrails and API i18n cleanup</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/063-ci-compliance-cleanup.html">063: CI compliance cleanup for test error handling</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/064-factory-reset-error-context.html">064: Factory reset error context and allow-path validation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/065-auth-mode-refresh.html">065: API key refresh and no-auth setup mode</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/066-factory-reset-sse-setup.html">066: Factory reset UX fallback and SSE setup gating</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/067-logs-ansi-rendering.html">067: Logs ANSI rendering and bounded buffer</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/068-agent-compliance-clippy-cargo.html">068: Agent compliance clippy cargo linting</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/069-docs-mdbook-mermaid-version.html">069: Pin mdbook-mermaid for docs builds</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/070-dashboard-ui-checklist.html">070: Dashboard UI checklist completion and auth/SSE hardening</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/071-libtorrent-native-fallback.html">071: Libtorrent native fallback for default CI</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/072-agent-compliance-refactor.html">072: Agent compliance refactor (UI + HTTP + Config Layout)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/073-ui-checklist-followups.html">073: UI checklist follow-ups: SSE detail refresh, labels shortcuts, strict i18n, and anymap removal</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/074-vendored-yewdux-latest-yew.html">074: Temporary vendoring of yewdux for latest Yew compatibility</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/075-coverage-gate-tests.html">075: Coverage gate tests for config loader and data toggles</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/076-hashbrown-multiple-versions-exception.html">076: Temporary clippy exception for hashbrown multiple versions</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/077-ui-menu-interactions.html">077: UI menu interactions</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/078-local-auth-bypass-guardrails.html">078: Local auth bypass guardrails</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/079-advisory-rustsec-2025-0141.html">079: Advisory RUSTSEC-2025-0141 temporary ignore</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/080-local-auth-bypass-reliability.html">080: Local auth bypass reliability</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/081-playwright-e2e-suite.html">081: Playwright E2E test suite</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/082-e2e-gate-and-selectors.html">082: E2E gate and selector stability</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/083-api-preflight-e2e.html">083: API preflight before UI E2E</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/084-e2e-api-coverage-temp-db.html">084: E2E API coverage with temp databases</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/085-e2e-openapi-client-and-coverage.html">085: E2E OpenAPI client and unified coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/086-default-local-auth-bypass.html">086: Default local auth bypass</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/087-local-network-auth-ranges.html">087: Local network auth ranges and settings validation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/088-live-sse-log-streaming.html">088: Live SSE log streaming</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/089-port-process-termination.html">089: Port process termination for dev tooling</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/090-ui-log-filters-and-shell-controls.html">090: UI log filters and shell controls</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/091-coverage-90-per-crate.html">091: Raise per-crate coverage gate to 90%</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/092-fsops-coverage-hardening.html">092: Fsops coverage hardening</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/093-ui-logic-extraction.html">093: UI logic extraction for testable components</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/094-ui-e2e-sharding.html">094: UI E2E sharding in workflows</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/095-untagged-image-dev-tag.html">095: Untagged images use dev tag</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/096-ui-e2e-coverage-aggregation.html">096: Aggregate UI E2E coverage for sharded runs</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/097-dev-release-flow.html">097: Dev prereleases and PR image previews</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/098-workflow-image-reuse.html">098: Reusable image build workflow</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/099-indexer-erd-single-tenant.html">099: Indexer ERD single-tenant and audit fields</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/100-sonar-scan-workflow-lcov.html">100: SonarQube workflow with root coverage LCOV</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/101-indexer-erd-checklist.html">101: Indexer ERD implementation checklist</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/102-indexer-core-schema.html">102: Indexer core schema foundations</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/103-indexer-definition-schema.html">103: Indexer definition schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/104-indexer-instance-schema.html">104: Indexer instance schema and RSS</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/105-indexer-secret-schema.html">105: Indexer secret schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/106-indexer-search-profile-torznab-schema.html">106: Indexer search profiles and Torznab schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/107-indexer-import-schema.html">107: Indexer import schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/108-indexer-rate-limit-cf-schema.html">108: Indexer rate limit and Cloudflare schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/109-indexer-policy-schema.html">109: Indexer policy schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/110-indexer-torznab-category-schema.html">110: Indexer Torznab category schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/111-indexer-connectivity-audit-schema.html">111: Indexer connectivity and audit schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/112-indexer-canonicalization-schema.html">112: Indexer canonicalization schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/113-indexer-search-request-schema.html">113: Indexer search request schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/114-indexer-scoring-schema.html">114: Indexer scoring schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/115-indexer-conflict-decision-schema.html">115: Indexer conflict and decision schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/116-indexer-user-action-schema.html">116: Indexer user action and acquisition schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/117-indexer-telemetry-reputation-schema.html">117: Indexer telemetry and reputation schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/118-indexer-job-schedule-schema.html">118: Indexer job schedule schema</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/119-indexer-fk-on-delete-rules.html">119: Indexer FK on-delete rules</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/120-indexer-seed-data.html">120: Indexer seed data and defaults</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/121-indexer-query-indexes.html">121: Indexer query indexes</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/122-indexer-deployment-init-proc.html">122: Indexer deployment initialization procedure</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/123-indexer-app-user-procs.html">123: Indexer app_user stored procedures</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/124-indexer-tag-procs.html">124: Indexer tag stored procedures</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/125-indexer-routing-policy-procs.html">125: Indexer routing policy stored procedures</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/126-indexer-cf-reset-proc.html">126: Indexer Cloudflare reset procedure</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/127-indexer-rate-limit-procs.html">127: Indexer rate limit stored procedures</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/128-indexer-instance-procs.html">128: Indexer instance stored procedures</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/129-indexer-category-mapping-procs.html">129: Indexer category mapping procedures</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/130-indexer-policy-set-procs.html">130: Indexer policy set procedures</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/131-indexer-search-profile-procs.html">131: Indexer search profile procedures</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/132-indexer-policy-rule-create-proc.html">132: Indexer policy rule create procedure</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/133-indexer-outbound-request-log-proc.html">133: Indexer outbound request log procedure</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/134-indexer-torznab-instance-state-procs.html">134: Indexer Torznab instance state procedures</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/135-indexer-conflict-resolution-procs.html">135: Indexer conflict resolution procedures</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/136-indexer-job-runner-procs.html">136: Indexer job runner procedures</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/137-indexer-search-request-cancel-proc.html">137: Indexer search request cancel procedure</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/138-indexer-search-run-procs.html">138: Indexer search run procedures</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/139-indexer-canonical-disambiguation-rule-proc.html">139: Indexer canonical disambiguation rule procedure</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/140-indexer-search-request-create-proc.html">140: Indexer search request create procedure</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/141-indexer-job-runner-followups.html">141: Indexer job runner follow-up procedures</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/142-indexer-executor-handoff-procs.html">142: Indexer executor handoff stored procedures</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/143-indexer-tag-api.html">143: Indexer tag API surface</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/144-task-indexer-proc-fixes.html">144: Task: Indexer procedure fixes (RSS apply, base score refresh, normalization)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/145-indexer-domain-mapping.html">145: Indexer domain mapping and DI boundaries</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/146-indexer-test-harness.html">146: Indexer stored-proc test harness</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/147-indexer-error-code-taxonomy.html">147: Indexer error-code taxonomy</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/148-indexer-v1-scope-routing.html">148: Indexer v1 scope enforcement</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/149-indexer-json-ban-verification.html">149: Indexer schema JSON ban verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/150-indexer-public-id-identity.html">150: Indexer public-id and bigint identity verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/151-indexer-soft-delete-verification.html">151: Indexer soft-delete coverage verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/152-indexer-audit-timestamps-verification.html">152: Indexer audit fields and timestamp defaults verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/153-indexer-api-boundary-public-ids.html">153: Indexer API boundary public-id verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/154-indexer-external-reference-public-ids.html">154: Indexer external reference public-id verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/155-indexer-system-sentinel-usage.html">155: Indexer system sentinel usage verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/156-indexer-text-caps-lowercase.html">156: Indexer text caps and lowercase key enforcement verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/157-indexer-normalized-columns.html">157: Indexer normalized column verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/158-indexer-hash-identity-rules.html">158: Indexer hash identity rules verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/159-indexer-secret-binding-only.html">159: Indexer secret binding linkage verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/160-indexer-single-tenant-scope.html">160: Indexer single-tenant scope verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/161-indexer-table-constraint-alignment.html">161: Indexer table/constraint alignment verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/162-indexer-per-table-notes-verification.html">162: Indexer per-table Notes verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/163-indexer-proc-error-codes.html">163: Indexer proc error-code alignment for key lookups</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/164-indexer-errors-normalization.html">164: Indexer error enums and normalization helpers verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/165-indexer-no-panics-result-only.html">165: Indexer result-only returns and no-panics verification</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/166-indexer-tryop-wrappers.html">166: Indexer tryOp wrappers for external operations</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/167-indexer-routing-policies.html">167: Indexer routing policy service and endpoints</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/168-indexer-definition-list.html">168: Indexer definition list endpoint</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/169-indexer-cf-state-get.html">169: Indexer CF state read endpoint</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/170-indexer-cf-state-e2e-coverage.html">170: Indexer CF state E2E coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/171-indexer-category-mapping-api.html">171: Indexer category mapping API endpoints</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/172-indexer-torznab-instance-api.html">172: Indexer Torznab instance API endpoints</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/173-indexer-search-profile-api.html">173: Indexer search profile API endpoints</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/174-indexer-import-jobs-api.html">174: Indexer import jobs API endpoints</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/175-indexer-import-cli.html">175: Indexer import jobs CLI commands</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/176-indexer-torznab-cli.html">176: Indexer Torznab CLI management</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/177-indexer-policy-cli.html">177: Indexer policy CLI management</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/178-indexer-instance-test-api.html">178: Indexer instance test API and CLI</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/179-indexer-allocation-guard.html">179: Indexer allocation safety guard</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/180-auth-prompt-dismissal.html">180: Auth prompt dismissal stability</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/181-allocation-live-memory-probe.html">181: Cross-platform allocation safety probe</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/182-indexer-pr-feedback.html">182: Indexer PR feedback follow-through</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/183-indexer-pr-feedback-allocations.html">183: Indexer PR feedback allocation follow-up</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/184-indexer-pr-feedback-more.html">184: Indexer PR feedback allocation caps</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/185-indexer-torznab-caps-endpoint.html">185: Indexer Torznab caps endpoint</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/186-indexer-torznab-download-and-allocation-guards.html">186: Indexer Torznab download and allocation guards</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/187-indexer-search-requests-api.html">187: Indexer search requests API and allocation guard refinements</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/188-indexer-search-request-auth-e2e.html">188: Indexer search request auth E2E coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/189-indexer-search-pages-api.html">189: Indexer search pages API</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/190-search-request-validation-tests.html">190: Search request validation tests</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/191-hash-identity-derivation-tests.html">191: Hash identity derivation tests</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/192-rate-limit-state-purge-test.html">192: Rate limit state purge test</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/193-job-schedule-completion.html">193: Job schedule completion updates</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/194-job-claim-locking-and-leases.html">194: Job claim locking and lease durations</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/195-policy-snapshot-gc-ordering.html">195: Policy snapshot GC ordering</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/196-retention-purge-context-cleanup.html">196: Retention purge context cleanup</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/197-indexer-connectivity-profile-refresh-rollups.html">197: Indexer connectivity profile refresh rollups</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/198-reputation-rollup-sample-thresholds.html">198: Reputation rollup sample thresholds</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/199-canonical-refresh-durable-source-cadence.html">199: Canonical refresh durable source cadence</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/200-canonical-prune-source-link-policy.html">200: Canonical prune source-link policy alignment</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/201-rss-poll-and-backfill-workflows.html">201: RSS poll and subscription backfill workflows</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/202-rss-scheduling-backoff-dedupe-validation.html">202: RSS scheduling, backoff, and dedupe validation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/203-rate-limit-token-bucket-and-rss-rate-limited-semantics.html">203: Rate limit token bucket and RSS rate-limited semantics</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/204-cf-state-transition-and-mitigation-validation.html">204: Cloudflare state transition and mitigation validation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/205-policy-snapshot-reuse-and-refcount-validation.html">205: Policy snapshot reuse and refcount validation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/206-policy-snapshot-gc-acceptance-coverage.html">206: Policy snapshot GC acceptance coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/207-derived-refresh-timing-and-caching-validation.html">207: Derived refresh timing and caching validation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/208-retention-and-rollup-job-window-validation.html">208: Retention and rollup job window validation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/209-retention-and-derived-refresh-strategy-coverage.html">209: Retention and derived refresh strategy coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/210-policy-rule-disable-enable-and-reorder-validation.html">210: Policy rule disable/enable and reorder validation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/211-search-result-observation-rules-validation.html">211: Search-result observation rules validation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/212-category-mapping-domain-filter-validation.html">212: Category mapping and domain filter validation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/213-indexer-observability-counters.html">213: Indexer observability counters for Torznab, search, and import jobs</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/214-indexer-request-span-coverage.html">214: Indexer request span coverage for Torznab, search, and import jobs</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/215-torznab-parity-integration-tests.html">215: Torznab parity integration tests for endpoint format and auth semantics</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/216-torznab-search-query-mapping-and-pagination.html">216: Torznab search query mapping and append-order pagination</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/217-torznab-download-redirect-acquisition-attempt-coverage.html">217: Torznab download redirect and acquisition-attempt coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/218-torznab-feed-category-emission-and-test-fixture-hardening.html">218: Torznab feed category emission and test fixture hardening</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/219-torznab-multi-category-domain-and-other-coverage.html">219: Torznab multi-category domain mapping and Other (8000) behavior coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/220-rate-limit-defaults-and-scope-enforcement-coverage.html">220: Rate-limit defaults and indexer/routing scope enforcement coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/221-search-run-retry-behavior-coverage.html">221: Search-run retry behavior coverage for rate-limited and transient errors</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/222-rss-cloudflare-state-transition-alignment.html">222: RSS Cloudflare state transition alignment with ERD</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/223-search-streaming-pages-terminal-seal.html">223: Search streaming pages terminal sealing and append-only ordering</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/224-search-dropped-source-audit-and-paging-exclusion.html">224: Search dropped-source audit persistence and paging exclusion</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/225-canonicalization-conflict-coverage.html">225: Canonicalization conflict coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/226-indexer-unit-test-domain-coverage.html">226: Indexer unit test domain coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/227-health-reputation-rollup-semantics.html">227: Health and reputation rollup semantics from outbound logs</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/228-search-zero-result-explainability.html">228: Search zero-result explainability</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/229-prowlarr-import-source-parity-tests.html">229: Prowlarr import source parity and dry-run coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/230-import-result-mapping-unmapped-coverage.html">230: Import result mapping and unmapped-definition coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/231-migration-parity-e2e-flows.html">231: Migration parity E2E flow coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/232-indexer-schema-and-procedure-catalog-tests.html">232: Indexer schema and procedure catalog verification tests</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/233-import-result-fidelity-snapshots.html">233: Import result fidelity snapshots</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/234-secret-binding-and-test-error-class-coverage.html">234: Secret binding and test error class coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/235-indexer-instance-create-uses-definition-slug-key.html">235: Indexer instance creation uses the public definition slug key</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/236-indexer-service-operation-metrics-and-spans.html">236: Indexer service operation metrics and spans</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/237-indexer-di-boundary-enforcement.html">237: Indexer dependency-injection boundary enforcement</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/238-manual-search-ui.html">238: Manual search UI</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/239-indexer-admin-console-ui.html">239: Indexer admin console UI</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/240-indexer-schedule-controls-ui.html">240: Indexer schedule controls UI</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/241-indexer-rss-management-ui.html">241: Indexer RSS management UI</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/242-indexer-connectivity-reputation-ui.html">242: Indexer connectivity and reputation UI</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/243-indexer-routing-policy-visibility.html">243: Indexer routing policy visibility</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/244-indexer-import-job-dashboard.html">244: Indexer import job dashboard</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/245-indexer-health-event-drilldown.html">245: Indexer health event drill-down</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/246-indexer-origin-only-error-logging.html">246: Indexer origin-only error logging</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/247-indexer-health-summary-panels.html">247: Indexer health summary panels</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/248-indexer-backup-restore.html">248: Indexer backup and restore</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/249-indexer-coexistence-rollback-acceptance.html">249: Indexer coexistence and rollback acceptance coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/250-indexer-domain-service-closeout.html">250: Indexer domain service closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/251-indexer-instance-category-overrides.html">251: Indexer instance category overrides</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/252-indexer-final-acceptance-closeout.html">252: Indexer final acceptance closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/253-indexer-health-notification-hooks.html">253: Indexer health notification hooks</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/254-indexer-app-sync-provisioning-ui.html">254: Indexer app sync provisioning UI</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/255-indexer-app-scoped-category-overrides.html">255: Indexer app-scoped category overrides</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/256-indexer-source-conflict-operator-ui.html">256: Indexer source conflict operator UI</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/257-indexer-cardigann-definition-import.html">257: Indexer Cardigann definition import</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/258-pr-review-closeout.html">258: PR review closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/259-pr-review-and-security-followup.html">259: PR review and security follow-up</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/260-pr-codeql-closeout.html">260: PR CodeQL closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/261-pr-security-and-thread-closeout.html">261: PR security and thread closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/262-pr-final-thread-closeout.html">262: PR final thread closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/263-sonarcloud-pr-issue-cleanup.html">263: SonarCloud PR issue cleanup and scope alignment</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/264-pr-unresolved-feedback-closeout.html">264: PR unresolved feedback closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/265-pr-feedback-boundary-validation-closeout.html">265: PR feedback boundary validation closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/266-pr-codeql-followup-on-instance-tag-bounds.html">266: PR CodeQL follow-up on instance tag bounds</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/267-indexer-maintenance-runtime.html">267: Indexer maintenance runtime</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/268-indexer-tag-secret-inventory.html">268: Indexer tag and secret inventory</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/269-indexer-operator-inventory-read-surfaces.html">269: Indexer operator inventory read surfaces</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/270-indexer-profile-policy-torznab-inventory.html">270: Indexer profile, policy, and Torznab inventory</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/271-indexer-cli-read-parity.html">271: Indexer CLI read parity</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/272-indexer-cli-operator-write-parity.html">272: Indexer CLI operator write parity</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/273-indexer-cli-mutation-parity-followup.html">273: Indexer CLI mutation parity follow-up</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/274-indexer-cli-health-notification-parity.html">274: Indexer CLI health-notification parity</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/275-pr-output-redaction-and-review-followup.html">275: PR output redaction and review follow-up</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/276-ci-cache-trim-for-runner-disk.html">276: CI cache trim for runner disk pressure</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/277-pr-review-handler-normalization-followup.html">277: PR review handler normalization follow-up</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/278-remediation-plan-implementation-closeout.html">278: Remediation plan implementation closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/279-remediation-plan-gap-closure.html">279: Remediation plan gap closure</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/280-pr-21-feedback-closeout.html">280: PR 21 feedback closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/281-pr-21-sonar-and-review-closeout.html">281: PR 21 Sonar and review closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/282-pr-21-final-feedback-closeout.html">282: PR 21 final feedback closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/283-pr-21-trivy-action-pin-refresh.html">283: PR 21 Trivy action pin refresh</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/284-instruction-refresh-and-sonar-scope.html">284: Instruction refresh and Sonar scope hardening</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/285-pr-19-review-and-lint-closeout.html">285: PR 19 review and lint closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/286-advisory-rustsec-2026-0097.html">286: Advisory RUSTSEC-2026-0097 temporary ignore</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/287-pr-19-policy-reconciliation.html">287: PR 19 policy reconciliation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/288-pr-19-openapi-test-portability.html">288: PR 19 OpenAPI test portability</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/289-pr-19-native-settings-snapshot-test-stability.html">289: PR 19 native settings snapshot test stability</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/290-pr-19-final-feedback-closeout.html">290: PR 19 final feedback closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/291-pr-19-sonar-quality-gate-restoration.html">291: PR 19 Sonar quality gate restoration</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/292-pr-19-review-timeout-stability.html">292: PR 19 review timeout stability</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/293-pr-19-github-action-sha-pinning.html">293: PR 19 GitHub Action SHA pinning</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/294-pr-19-review-feedback-closeout.html">294: PR 19 review feedback closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/295-dependency-bump-rollup.html">295: Dependency bump rollup</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/296-helm-chart-release-publishing.html">296: Helm chart release publishing</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/297-helm-feedback-and-sonar-closeout.html">297: Helm feedback and Sonar closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/298-ci-workflow-permissions-regression.html">298: CI workflow permissions regression</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/299-trivy-config-baseline.html">299: Trivy config baseline</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/300-trivy-container-and-sonar-pgsql-config.html">300: Trivy container and Sonar PGSQL config</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/301-security-dependency-refresh-for-pr-25.html">301: Security dependency refresh for PR 25</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/302-pr-validation-and-main-release-workflow-split.html">302: PR validation and main release workflow split</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/303-release-tag-image-job-dependency-split.html">303: Release tag image job dependency split</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/304-pr-25-deny-exception-and-sonar-hotspot-closeout.html">304: PR 25 deny exception and Sonar hotspot closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/305-pr-25-prerelease-tag-release-guard.html">305: PR 25 prerelease tag release guard</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/306-semantic-release-prepare-template-fix.html">306: Semantic release prepare template fix</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/307-ci-oras-setup-action-refresh.html">307: CI ORAS setup action refresh</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/308-pr-build-images-dev-helm-publish.html">308: PR workflow Helm and Sonar consolidation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/309-ghcr-helm-namespace-derivation.html">309: GHCR Helm namespace derivation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/310-pr-helm-review-followups.html">310: PR Helm review follow-ups</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/311-ghcr-helm-github-token-auth.html">311: GHCR Helm GitHub token authentication</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/312-artifacthub-oci-repository-alignment.html">312: Artifact Hub OCI repository alignment</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/313-trivy-sarif-category-and-ghcr-token-alignment.html">313: Trivy SARIF category and GHCR token alignment</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/314-artifacthub-verification-and-official-readiness.html">314: Artifact Hub verification and official readiness</a></span></li></ol></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split('#')[0].split('?')[0];
        if (current_page.endsWith('/')) {
            current_page += 'index.html';
        }
        const links = Array.prototype.slice.call(this.querySelectorAll('a'));
        const l = links.length;
        for (let i = 0; i < l; ++i) {
            const link = links[i];
            const href = link.getAttribute('href');
            if (href && !href.startsWith('#') && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The 'index' page is supposed to alias the first chapter in the book.
            if (link.href === current_page
                || i === 0
                && path_to_root === ''
                && current_page.endsWith('/index.html')) {
                link.classList.add('active');
                let parent = link.parentElement;
                while (parent) {
                    if (parent.tagName === 'LI' && parent.classList.contains('chapter-item')) {
                        parent.classList.add('expanded');
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', e => {
            if (e.target.tagName === 'A') {
                const clientRect = e.target.getBoundingClientRect();
                const sidebarRect = this.getBoundingClientRect();
                sessionStorage.setItem('sidebar-scroll-offset', clientRect.top - sidebarRect.top);
            }
        }, { passive: true });
        const sidebarScrollOffset = sessionStorage.getItem('sidebar-scroll-offset');
        sessionStorage.removeItem('sidebar-scroll-offset');
        if (sidebarScrollOffset !== null) {
            // preserve sidebar scroll position when navigating via links within sidebar
            const activeSection = this.querySelector('.active');
            if (activeSection) {
                const clientRect = activeSection.getBoundingClientRect();
                const sidebarRect = this.getBoundingClientRect();
                const currentOffset = clientRect.top - sidebarRect.top;
                this.scrollTop += currentOffset - parseFloat(sidebarScrollOffset);
            }
        } else {
            // scroll sidebar to current active section when navigating via
            // 'next/previous chapter' buttons
            const activeSection = document.querySelector('#mdbook-sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        const sidebarAnchorToggles = document.querySelectorAll('.chapter-fold-toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(el => {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define('mdbook-sidebar-scrollbox', MDBookSidebarScrollbox);


// ---------------------------------------------------------------------------
// Support for dynamically adding headers to the sidebar.

(function() {
    // This is used to detect which direction the page has scrolled since the
    // last scroll event.
    let lastKnownScrollPosition = 0;
    // This is the threshold in px from the top of the screen where it will
    // consider a header the "current" header when scrolling down.
    const defaultDownThreshold = 150;
    // Same as defaultDownThreshold, except when scrolling up.
    const defaultUpThreshold = 300;
    // The threshold is a virtual horizontal line on the screen where it
    // considers the "current" header to be above the line. The threshold is
    // modified dynamically to handle headers that are near the bottom of the
    // screen, and to slightly offset the behavior when scrolling up vs down.
    let threshold = defaultDownThreshold;
    // This is used to disable updates while scrolling. This is needed when
    // clicking the header in the sidebar, which triggers a scroll event. It
    // is somewhat finicky to detect when the scroll has finished, so this
    // uses a relatively dumb system of disabling scroll updates for a short
    // time after the click.
    let disableScroll = false;
    // Array of header elements on the page.
    let headers;
    // Array of li elements that are initially collapsed headers in the sidebar.
    // I'm not sure why eslint seems to have a false positive here.
    // eslint-disable-next-line prefer-const
    let headerToggles = [];
    // This is a debugging tool for the threshold which you can enable in the console.
    let thresholdDebug = false;

    // Updates the threshold based on the scroll position.
    function updateThreshold() {
        const scrollTop = window.pageYOffset || document.documentElement.scrollTop;
        const windowHeight = window.innerHeight;
        const documentHeight = document.documentElement.scrollHeight;

        // The number of pixels below the viewport, at most documentHeight.
        // This is used to push the threshold down to the bottom of the page
        // as the user scrolls towards the bottom.
        const pixelsBelow = Math.max(0, documentHeight - (scrollTop + windowHeight));
        // The number of pixels above the viewport, at least defaultDownThreshold.
        // Similar to pixelsBelow, this is used to push the threshold back towards
        // the top when reaching the top of the page.
        const pixelsAbove = Math.max(0, defaultDownThreshold - scrollTop);
        // How much the threshold should be offset once it gets close to the
        // bottom of the page.
        const bottomAdd = Math.max(0, windowHeight - pixelsBelow - defaultDownThreshold);
        let adjustedBottomAdd = bottomAdd;

        // Adjusts bottomAdd for a small document. The calculation above
        // assumes the document is at least twice the windowheight in size. If
        // it is less than that, then bottomAdd needs to be shrunk
        // proportional to the difference in size.
        if (documentHeight < windowHeight * 2) {
            const maxPixelsBelow = documentHeight - windowHeight;
            const t = 1 - pixelsBelow / Math.max(1, maxPixelsBelow);
            const clamp = Math.max(0, Math.min(1, t));
            adjustedBottomAdd *= clamp;
        }

        let scrollingDown = true;
        if (scrollTop < lastKnownScrollPosition) {
            scrollingDown = false;
        }

        if (scrollingDown) {
            // When scrolling down, move the threshold up towards the default
            // downwards threshold position. If near the bottom of the page,
            // adjustedBottomAdd will offset the threshold towards the bottom
            // of the page.
            const amountScrolledDown = scrollTop - lastKnownScrollPosition;
            const adjustedDefault = defaultDownThreshold + adjustedBottomAdd;
            threshold = Math.max(adjustedDefault, threshold - amountScrolledDown);
        } else {
            // When scrolling up, move the threshold down towards the default
            // upwards threshold position. If near the bottom of the page,
            // quickly transition the threshold back up where it normally
            // belongs.
            const amountScrolledUp = lastKnownScrollPosition - scrollTop;
            const adjustedDefault = defaultUpThreshold - pixelsAbove
                + Math.max(0, adjustedBottomAdd - defaultDownThreshold);
            threshold = Math.min(adjustedDefault, threshold + amountScrolledUp);
        }

        if (documentHeight <= windowHeight) {
            threshold = 0;
        }

        if (thresholdDebug) {
            const id = 'mdbook-threshold-debug-data';
            let data = document.getElementById(id);
            if (data === null) {
                data = document.createElement('div');
                data.id = id;
                data.style.cssText = `
                    position: fixed;
                    top: 50px;
                    right: 10px;
                    background-color: 0xeeeeee;
                    z-index: 9999;
                    pointer-events: none;
                `;
                document.body.appendChild(data);
            }
            data.innerHTML = `
                <table>
                  <tr><td>documentHeight</td><td>${documentHeight.toFixed(1)}</td></tr>
                  <tr><td>windowHeight</td><td>${windowHeight.toFixed(1)}</td></tr>
                  <tr><td>scrollTop</td><td>${scrollTop.toFixed(1)}</td></tr>
                  <tr><td>pixelsAbove</td><td>${pixelsAbove.toFixed(1)}</td></tr>
                  <tr><td>pixelsBelow</td><td>${pixelsBelow.toFixed(1)}</td></tr>
                  <tr><td>bottomAdd</td><td>${bottomAdd.toFixed(1)}</td></tr>
                  <tr><td>adjustedBottomAdd</td><td>${adjustedBottomAdd.toFixed(1)}</td></tr>
                  <tr><td>scrollingDown</td><td>${scrollingDown}</td></tr>
                  <tr><td>threshold</td><td>${threshold.toFixed(1)}</td></tr>
                </table>
            `;
            drawDebugLine();
        }

        lastKnownScrollPosition = scrollTop;
    }

    function drawDebugLine() {
        if (!document.body) {
            return;
        }
        const id = 'mdbook-threshold-debug-line';
        const existingLine = document.getElementById(id);
        if (existingLine) {
            existingLine.remove();
        }
        const line = document.createElement('div');
        line.id = id;
        line.style.cssText = `
            position: fixed;
            top: ${threshold}px;
            left: 0;
            width: 100vw;
            height: 2px;
            background-color: red;
            z-index: 9999;
            pointer-events: none;
        `;
        document.body.appendChild(line);
    }

    function mdbookEnableThresholdDebug() {
        thresholdDebug = true;
        updateThreshold();
        drawDebugLine();
    }

    window.mdbookEnableThresholdDebug = mdbookEnableThresholdDebug;

    // Updates which headers in the sidebar should be expanded. If the current
    // header is inside a collapsed group, then it, and all its parents should
    // be expanded.
    function updateHeaderExpanded(currentA) {
        // Add expanded to all header-item li ancestors.
        let current = currentA.parentElement;
        while (current) {
            if (current.tagName === 'LI' && current.classList.contains('header-item')) {
                current.classList.add('expanded');
            }
            current = current.parentElement;
        }
    }

    // Updates which header is marked as the "current" header in the sidebar.
    // This is done with a virtual Y threshold, where headers at or below
    // that line will be considered the current one.
    function updateCurrentHeader() {
        if (!headers || !headers.length) {
            return;
        }

        // Reset the classes, which will be rebuilt below.
        const els = document.getElementsByClassName('current-header');
        for (const el of els) {
            el.classList.remove('current-header');
        }
        for (const toggle of headerToggles) {
            toggle.classList.remove('expanded');
        }

        // Find the last header that is above the threshold.
        let lastHeader = null;
        for (const header of headers) {
            const rect = header.getBoundingClientRect();
            if (rect.top <= threshold) {
                lastHeader = header;
            } else {
                break;
            }
        }
        if (lastHeader === null) {
            lastHeader = headers[0];
            const rect = lastHeader.getBoundingClientRect();
            const windowHeight = window.innerHeight;
            if (rect.top >= windowHeight) {
                return;
            }
        }

        // Get the anchor in the summary.
        const href = '#' + lastHeader.id;
        const a = [...document.querySelectorAll('.header-in-summary')]
            .find(element => element.getAttribute('href') === href);
        if (!a) {
            return;
        }

        a.classList.add('current-header');

        updateHeaderExpanded(a);
    }

    // Updates which header is "current" based on the threshold line.
    function reloadCurrentHeader() {
        if (disableScroll) {
            return;
        }
        updateThreshold();
        updateCurrentHeader();
    }


    // When clicking on a header in the sidebar, this adjusts the threshold so
    // that it is located next to the header. This is so that header becomes
    // "current".
    function headerThresholdClick(event) {
        // See disableScroll description why this is done.
        disableScroll = true;
        setTimeout(() => {
            disableScroll = false;
        }, 100);
        // requestAnimationFrame is used to delay the update of the "current"
        // header until after the scroll is done, and the header is in the new
        // position.
        requestAnimationFrame(() => {
            requestAnimationFrame(() => {
                // Closest is needed because if it has child elements like <code>.
                const a = event.target.closest('a');
                const href = a.getAttribute('href');
                const targetId = href.substring(1);
                const targetElement = document.getElementById(targetId);
                if (targetElement) {
                    threshold = targetElement.getBoundingClientRect().bottom;
                    updateCurrentHeader();
                }
            });
        });
    }

    // Takes the nodes from the given head and copies them over to the
    // destination, along with some filtering.
    function filterHeader(source, dest) {
        const clone = source.cloneNode(true);
        clone.querySelectorAll('mark').forEach(mark => {
            mark.replaceWith(...mark.childNodes);
        });
        dest.append(...clone.childNodes);
    }

    // Scans page for headers and adds them to the sidebar.
    document.addEventListener('DOMContentLoaded', function() {
        const activeSection = document.querySelector('#mdbook-sidebar .active');
        if (activeSection === null) {
            return;
        }

        const main = document.getElementsByTagName('main')[0];
        headers = Array.from(main.querySelectorAll('h2, h3, h4, h5, h6'))
            .filter(h => h.id !== '' && h.children.length && h.children[0].tagName === 'A');

        if (headers.length === 0) {
            return;
        }

        // Build a tree of headers in the sidebar.

        const stack = [];

        const firstLevel = parseInt(headers[0].tagName.charAt(1));
        for (let i = 1; i < firstLevel; i++) {
            const ol = document.createElement('ol');
            ol.classList.add('section');
            if (stack.length > 0) {
                stack[stack.length - 1].ol.appendChild(ol);
            }
            stack.push({level: i + 1, ol: ol});
        }

        // The level where it will start folding deeply nested headers.
        const foldLevel = 3;

        for (let i = 0; i < headers.length; i++) {
            const header = headers[i];
            const level = parseInt(header.tagName.charAt(1));

            const currentLevel = stack[stack.length - 1].level;
            if (level > currentLevel) {
                // Begin nesting to this level.
                for (let nextLevel = currentLevel + 1; nextLevel <= level; nextLevel++) {
                    const ol = document.createElement('ol');
                    ol.classList.add('section');
                    const last = stack[stack.length - 1];
                    const lastChild = last.ol.lastChild;
                    // Handle the case where jumping more than one nesting
                    // level, which doesn't have a list item to place this new
                    // list inside of.
                    if (lastChild) {
                        lastChild.appendChild(ol);
                    } else {
                        last.ol.appendChild(ol);
                    }
                    stack.push({level: nextLevel, ol: ol});
                }
            } else if (level < currentLevel) {
                while (stack.length > 1 && stack[stack.length - 1].level > level) {
                    stack.pop();
                }
            }

            const li = document.createElement('li');
            li.classList.add('header-item');
            li.classList.add('expanded');
            if (level < foldLevel) {
                li.classList.add('expanded');
            }
            const span = document.createElement('span');
            span.classList.add('chapter-link-wrapper');
            const a = document.createElement('a');
            span.appendChild(a);
            a.href = '#' + header.id;
            a.classList.add('header-in-summary');
            filterHeader(header.children[0], a);
            a.addEventListener('click', headerThresholdClick);
            const nextHeader = headers[i + 1];
            if (nextHeader !== undefined) {
                const nextLevel = parseInt(nextHeader.tagName.charAt(1));
                if (nextLevel > level && level >= foldLevel) {
                    const toggle = document.createElement('a');
                    toggle.classList.add('chapter-fold-toggle');
                    toggle.classList.add('header-toggle');
                    toggle.addEventListener('click', () => {
                        li.classList.toggle('expanded');
                    });
                    const toggleDiv = document.createElement('div');
                    toggleDiv.textContent = '❱';
                    toggle.appendChild(toggleDiv);
                    span.appendChild(toggle);
                    headerToggles.push(li);
                }
            }
            li.appendChild(span);

            const currentParent = stack[stack.length - 1];
            currentParent.ol.appendChild(li);
        }

        const onThisPage = document.createElement('div');
        onThisPage.classList.add('on-this-page');
        onThisPage.append(stack[0].ol);
        const activeItemSpan = activeSection.parentElement;
        activeItemSpan.after(onThisPage);
    });

    document.addEventListener('DOMContentLoaded', reloadCurrentHeader);
    document.addEventListener('scroll', reloadCurrentHeader, { passive: true });
})();

