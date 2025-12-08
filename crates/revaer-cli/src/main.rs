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

//! Thin entrypoint that delegates to the library for CLI execution.

/// Parses CLI arguments and executes the requested command.
#[tokio::main]
async fn main() {
    let exit_code = revaer_cli::run().await;
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}
