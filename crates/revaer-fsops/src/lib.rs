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
#![allow(clippy::multiple_crate_versions)]

//! Filesystem post-processing pipeline for completed torrents.
//! Layout: `model/` (request types), `error.rs` (error types), `service/` (pipeline + IO).

pub mod error;
pub mod model;
pub mod service;

pub use error::{FsOpsError, FsOpsResult};
pub use model::FsOpsRequest;
pub use service::*;
