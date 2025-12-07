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

//! Administrative CLI for interacting with a Revaer server instance.
//!
//! Layout: `cli.rs` (argument parsing, command dispatch) with a thin `main.rs`
//! that delegates to `run()`.

pub mod cli;

pub use cli::run;
