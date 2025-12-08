#![forbid(unsafe_code)]
#![deny(
    warnings,
    dead_code,
    unused,
    unused_imports,
    unused_must_use,
    unreachable_pub,
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]

//! Thin CLI entrypoint that delegates to the library implementation.

use std::path::PathBuf;

use anyhow::Result;
use revaer_doc_indexer::run;

/// Entry point for generating the documentation manifest and summaries files.
fn main() -> Result<()> {
    let docs_root = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("docs"), PathBuf::from);

    let schema_path = docs_root.join("llm").join("schema.json");
    run(&docs_root, &schema_path)
}
