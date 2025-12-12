// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

(() => {
    const darkThemes = ['ayu', 'navy', 'coal'];
    const lightThemes = ['light', 'rust'];

    const normalizeMermaidBlocks = () => {
        const targets = [];

        // Convert <pre class="mermaid"> and fenced code blocks into <div class="mermaid">
        const rewrite = (node) => {
            const container = document.createElement('div');
            container.className = 'mermaid';
            container.textContent = node.textContent;
            node.replaceWith(container);
            targets.push(container);
        };

        document.querySelectorAll('pre.mermaid').forEach(rewrite);
        document.querySelectorAll('code.language-mermaid').forEach((code) => {
            rewrite(code.parentElement?.tagName === 'PRE' ? code.parentElement : code);
        });
        document.querySelectorAll('div.mermaid').forEach((div) => targets.push(div));

        return targets;
    };

    const pickTheme = () => {
        const classList = document.documentElement.classList;
        for (const cssClass of classList) {
            if (darkThemes.includes(cssClass)) {
                return { theme: 'dark', lastThemeWasLight: false };
            }
        }
        return { theme: 'default', lastThemeWasLight: true };
    };

    const render = () => {
        if (!window.mermaid) {
            console.error('Mermaid failed to load; diagrams will stay unrendered.');
            return;
        }

        const targets = normalizeMermaidBlocks();
        if (targets.length === 0) {
            return;
        }

        const { theme, lastThemeWasLight } = pickTheme();
        mermaid.initialize({ startOnLoad: false, theme });
        mermaid
            .run({ querySelector: '.mermaid' })
            .catch((err) => console.error('Failed to render mermaid diagrams', err));

        // Simplest way to make mermaid re-render the diagrams in the new theme is via refreshing the page
        for (const darkTheme of darkThemes) {
            document.getElementById(darkTheme)?.addEventListener('click', () => {
                if (lastThemeWasLight) {
                    window.location.reload();
                }
            });
        }

        for (const lightTheme of lightThemes) {
            document.getElementById(lightTheme)?.addEventListener('click', () => {
                if (!lastThemeWasLight) {
                    window.location.reload();
                }
            });
        }
    };

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', render, { once: true });
    } else {
        render();
    }
})();
