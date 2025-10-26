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
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded "><a href="index.html">Overview</a></li><li class="chapter-item expanded "><a href="phase-one-roadmap.html">Phase One Roadmap</a></li><li class="chapter-item expanded "><a href="phase-one-remaining-spec.html">Phase One Remaining Spec</a></li><li class="chapter-item expanded "><a href="runbook.html">Runbook</a></li><li class="chapter-item expanded "><a href="release-checklist.html">Release Checklist</a></li><li class="chapter-item expanded "><a href="platform/configuration.html">Configuration Surface</a></li><li class="chapter-item expanded "><a href="platform/api.html">HTTP API</a></li><li class="chapter-item expanded "><a href="platform/cli.html">CLI Reference</a></li><li class="chapter-item expanded "><a href="api/index.html">API Overview</a></li><li class="chapter-item expanded "><a href="api/openapi.html">OpenAPI Reference</a></li><li class="chapter-item expanded "><a href="adr/index.html">Working with ADRs</a></li><li class="chapter-item expanded "><a href="adr/001-configuration-revisioning.html">001: Configuration Revisioning</a></li><li class="chapter-item expanded "><a href="adr/002-setup-token-lifecycle.html">002: Setup Token Lifecycle</a></li><li class="chapter-item expanded "><a href="adr/003-libtorrent-session-runner.html">003: Libtorrent Session Runner</a></li><li class="chapter-item expanded "><a href="adr/004-phase-one-delivery.html">004: Phase One Delivery</a></li><li class="chapter-item expanded "><a href="adr/005-fsops-pipeline.html">005: Filesystem Operations Pipeline</a></li><li class="chapter-item expanded "><a href="adr/006-api-cli-contract.html">006: API and CLI Contract</a></li><li class="chapter-item expanded "><a href="adr/007-security-posture.html">007: Security Posture</a></li><li class="chapter-item expanded "><a href="adr/008-phase-one-remaining-task.html">008: Phase One Remaining Task</a></li><li class="chapter-item expanded "><a href="adr/template.html">ADR Template</a></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split("#")[0].split("?")[0];
        if (current_page.endsWith("/")) {
            current_page += "index.html";
        }
        var links = Array.prototype.slice.call(this.querySelectorAll("a"));
        var l = links.length;
        for (var i = 0; i < l; ++i) {
            var link = links[i];
            var href = link.getAttribute("href");
            if (href && !href.startsWith("#") && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The "index" page is supposed to alias the first chapter in the book.
            if (link.href === current_page || (i === 0 && path_to_root === "" && current_page.endsWith("/index.html"))) {
                link.classList.add("active");
                var parent = link.parentElement;
                if (parent && parent.classList.contains("chapter-item")) {
                    parent.classList.add("expanded");
                }
                while (parent) {
                    if (parent.tagName === "LI" && parent.previousElementSibling) {
                        if (parent.previousElementSibling.classList.contains("chapter-item")) {
                            parent.previousElementSibling.classList.add("expanded");
                        }
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', function(e) {
            if (e.target.tagName === 'A') {
                sessionStorage.setItem('sidebar-scroll', this.scrollTop);
            }
        }, { passive: true });
        var sidebarScrollTop = sessionStorage.getItem('sidebar-scroll');
        sessionStorage.removeItem('sidebar-scroll');
        if (sidebarScrollTop) {
            // preserve sidebar scroll position when navigating via links within sidebar
            this.scrollTop = sidebarScrollTop;
        } else {
            // scroll sidebar to current active section when navigating via "next/previous chapter" buttons
            var activeSection = document.querySelector('#sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        var sidebarAnchorToggles = document.querySelectorAll('#sidebar a.toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(function (el) {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define("mdbook-sidebar-scrollbox", MDBookSidebarScrollbox);
