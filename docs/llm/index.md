# LLM Integration

The documentation site publishes machine-readable JSON manifests to help ChatGPT and other LLMs index and search the content. These files are static and live alongside the built docs, so LLM tooling can fetch them directly.

- **[`manifest.json`](manifest.json)** — Master index of documentation entries with IDs, titles, tags, and canonical URLs. Shared so LLMs can discover the page graph without crawling the entire site.
- **[`summaries.json`](summaries.json)** — Concise per-entry summaries extracted from the docs. Shared to provide quick context for answers and improve retrieval quality.
- **[`schema.json`](schema.json)** — JSON Schema describing the manifest/summaries format. Shared so LLM integrators can validate and evolve their tooling against a stable contract.

These files are generated during `just docs` and hosted with the site; consuming them keeps search fast and avoids scraping overhead.
