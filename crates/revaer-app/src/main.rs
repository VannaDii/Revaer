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
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]

//! Binary entrypoint that wires the Revaer services together and launches the
//! async orchestrators.

use anyhow::Result;
use revaer_app::run_app;

/// Bootstraps the Revaer application and blocks until shutdown.
#[tokio::main]
async fn main() -> Result<()> {
    run_app().await
}
