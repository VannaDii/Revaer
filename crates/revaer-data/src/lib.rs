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

//! Shared data access layer for Revaer: migrations, stored procedures, and repositories.

pub mod config;
pub mod error;
pub mod runtime;

pub use error::{DataError, Result as DataResult};
pub use runtime::RuntimeStore;
