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
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="index.html">Overview</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="phase-one-roadmap.html">Phase One Roadmap</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="phase-one-remaining-spec.html">Phase One Remaining Spec</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="runbook.html">Runbook</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="release-checklist.html">Release Checklist</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="ui/index.html">Web UI - Phase 1</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="ui/flows.html">Web UI Flows</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="platform/configuration.html">Configuration Surface</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="platform/api.html">HTTP API</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="platform/cli.html">CLI Reference</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="platform/torrent-flows.html">Torrent Flows</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="platform/native-tests.html">Native Libtorrent Tests</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="api/index.html">API Overview</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="api/openapi.html">OpenAPI Reference</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/index.html">ADRs</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/template.html">ADR Template</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/001-configuration-revisioning.html">001: Configuration revisioning</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/002-setup-token-lifecycle.html">002: Setup token lifecycle</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/003-libtorrent-session-runner.html">003: Libtorrent session runner</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/004-phase-one-delivery.html">004: Phase one delivery</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/005-fsops-pipeline.html">005: FS operations pipeline</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/006-api-cli-contract.html">006: API/CLI contract</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/007-security-posture.html">007: Security posture</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/008-phase-one-remaining-task.html">008: Remaining phase-one tasks</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/009-fsops-permission-hardening.html">009: FS ops permission hardening</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/010-agent-compliance-sweep.html">010: Agent compliance sweep</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/011-coverage-hardening-phase-two.html">011: Coverage hardening</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/012-agent-compliance-refresh.html">012: Agent compliance refresh</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/013-runtime-persistence.html">013: Runtime persistence</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/014-data-access-layer.html">014: Data access layer</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/015-agent-compliance-hardening.html">015: Agent compliance hardening</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/016-libtorrent-restoration.html">016: Libtorrent restoration</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/017-sqlx-named-bind.html">017: Avoid sqlx-named-bind</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/018-retire-testcontainers.html">018: Retire testcontainers</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/019-advisory-rustsec-2024-0370.html">019: Advisory RUSTSEC-2024-0370 temporary ignore</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/020-torrent-engine-precursors.html">020: Torrent engine precursor hardening</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/021-torrent-precursor-enforcement.html">021: Torrent precursor enforcement</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/022-torrent-settings-parity.html">022: Torrent settings parity and observability</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/023-tracker-config-wiring.html">023: Tracker config wiring and persistence</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/024-seeding-stop-criteria.html">024: Seeding stop criteria and overrides</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/025-seed-mode-add-as-complete.html">025: Seed mode admission with optional hash sampling</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/026-queue-auto-managed-and-pex.html">026: Queue auto-managed defaults and PEX threading</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/027-choking-and-super-seeding.html">027: Choking strategy and super-seeding configuration</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/028-qbittorrent-parity-and-tracker-tls.html">028: qBittorrent parity and tracker TLS wiring</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/029-torrent-authoring-labels-and-metadata.html">029: Torrent authoring, labels, and metadata updates</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/030-migration-consolidation.html">030: Migration consolidation for initial setup</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/031-ui-asset-sync.html">031: UI Nexus asset sync tooling</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/032-torrent-ffi-audit-closeout.html">032: Torrent FFI audit closeout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/033-ui-sse-auth-setup.html">033: UI SSE + auth/setup wiring</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/034-ui-sse-store-apiclient.html">034: UI SSE normalization and ApiClient singleton</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/035-advisory-rustsec-2021-0065.html">035: Advisory RUSTSEC-2021-0065 temporary ignore</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/036-asset-sync-test-stability.html">036: Asset sync test stability under parallel runs</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/037-ui-row-slices-system-rates.html">037: UI row slices and system-rate store wiring</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/038-ui-api-models-filters-paging.html">038: UI shared API models and torrent query paging state</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/039-ui-store-api-rate-limit.html">039: UI store, API coverage, and rate-limit retries</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/040-ui-labels-policy.html">040: UI label policy editor and API wiring</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/041-ui-health-shortcuts.html">041: UI health view and label shortcuts</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/042-ui-metrics-copy.html">042: UI metrics copy button</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/043-ui-settings-bypass-auth.html">043: UI settings bypass local auth toggle</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/044-ui-api-client-options-selection.html">044: UI ApiClient torrent options/selection endpoints</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/045-ui-icon-system.html">045: UI icon components and icon button standardization</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/046-ui-torrent-filters-pagination.html">046: UI torrent filters, pagination, and URL sync</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/047-ui-torrent-updated-column.html">047: UI torrent list updated timestamp column</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/048-ui-torrent-actions-bulk-controls.html">048: UI torrent row actions, bulk controls, and rate/remove dialogs</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/049-ui-detail-overview-files-options.html">049: UI detail drawer overview/files/options</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/050-ui-torrent-fab-create-modals.html">050: UI torrent FAB, add modal, and create-torrent authoring flow</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/051-ui-api-models-primitives.html">051: UI shared API models and UX primitives</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/052-ui-nexus-dashboard.html">052: UI dashboard migration to Nexus vendor layout</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/053-ui-dashboard-hardline-rebuild.html">053: UI dashboard hardline rebuild</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/054-ui-dashboard-nexus-parity.html">054: UI dashboard Nexus parity tweaks</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/055-factory-reset-bootstrap-api-key.html">055: Factory reset and bootstrap API key</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/056-factory-reset-bootstrap-auth-fallback.html">056: Factory reset auth fallback when no API keys exist</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/057-ui-settings-tabs-controls.html">057: UI settings tabs and editor controls</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/058-settings-logs-fs-browser.html">058: UI settings controls, logs stream, and filesystem browser</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/059-migration-rebaseline.html">059: Migration rebaseline and JSON backfill guardrails</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/060-auth-expiry-error-context.html">060: Auth expiry enforcement and structured error context</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/061-api-i18n-openapi-assets.html">061: API error i18n and OpenAPI asset constants</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/062-eventbus-publish-guardrails.html">062: Event bus publish guardrails and API i18n cleanup</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/063-ci-compliance-cleanup.html">063: CI compliance cleanup for test error handling</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/064-factory-reset-error-context.html">064: Factory reset error context and allow-path validation</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/065-auth-mode-refresh.html">065: API key refresh and no-auth setup mode</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/066-factory-reset-sse-setup.html">066: Factory reset UX fallback and SSE setup gating</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/067-logs-ansi-rendering.html">067: Logs ANSI rendering and bounded buffer</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/068-agent-compliance-clippy-cargo.html">068: Agent compliance clippy cargo linting</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/069-docs-mdbook-mermaid-version.html">069: Pin mdbook-mermaid for docs builds</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/070-dashboard-ui-checklist.html">070: Dashboard UI checklist completion and auth/SSE hardening</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="adr/071-libtorrent-native-fallback.html">071: Libtorrent native fallback for default CI</a></span></li></ol></li></ol>';
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

