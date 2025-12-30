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

//! Filesystem post-processing pipeline for completed torrents.
//! Layout: `error.rs` (error types), `service.rs` (pipeline + IO).

pub mod error;
pub mod service;

pub use error::{FsOpsError, FsOpsResult};
pub use service::*;
