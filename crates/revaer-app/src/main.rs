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

//! Binary entrypoint that wires the Revaer services together and launches the
//! async orchestrators.

use revaer_app::{AppResult, run_app};

/// Bootstraps the Revaer application and blocks until shutdown.
#[tokio::main]
async fn main() -> AppResult<()> {
    run_app().await
}
