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
//! CLI entrypoint for the Nexus asset sync tool.
//!
//! # Design
//! Delegates to the library implementation and surfaces errors via `anyhow`.

use anyhow::Result;

fn main() -> Result<()> {
    asset_sync::run()?;
    Ok(())
}
