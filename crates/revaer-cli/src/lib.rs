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
#![allow(clippy::redundant_pub_crate)]

//! Administrative CLI for interacting with a Revaer server instance.
//!
//! Layout:
//! - `cli.rs`: argument parsing and command dispatch
//! - `commands/`: command handlers grouped by concern
//! - `client.rs`: shared HTTP client, errors, and telemetry helpers
//! - `output.rs`: renderers and formatting helpers
//! - `main.rs`: thin entrypoint delegating to `run()`

pub(crate) mod cli;
pub(crate) mod client;
pub(crate) mod commands;
pub(crate) mod output;

pub use cli::run;
