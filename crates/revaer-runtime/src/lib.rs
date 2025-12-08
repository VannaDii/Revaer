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

//! Runtime persistence facade for torrent and filesystem job tracking.
//! Layout: `runtime.rs` (re-exports of the data-layer runtime store).

pub mod runtime;

pub use runtime::*;
