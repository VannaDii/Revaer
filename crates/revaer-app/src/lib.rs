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

//! Revaer application bootstrap wiring.
//!
//! Layout: `bootstrap.rs` (service wiring), `orchestrator.rs` (torrent/fsops orchestration).

/// Application bootstrap and environment loading.
pub mod bootstrap;
/// Engine profile normalisation and runtime mapping.
#[cfg(feature = "libtorrent")]
pub mod engine_config;
/// Torrent orchestrator wiring.
#[cfg(feature = "libtorrent")]
pub mod orchestrator;

pub use bootstrap::run_app;
