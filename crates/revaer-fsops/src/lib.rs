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
//! Layout: `service.rs` (pipeline + IO), future `model/` and `policy/` modules to follow.

pub mod service;

pub use service::*;
