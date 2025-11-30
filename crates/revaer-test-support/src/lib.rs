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
#![allow(clippy::module_name_repetitions, clippy::multiple_crate_versions)]
#![allow(unexpected_cfgs)]

//! Shared test helpers used across integration suites.
//! Layout: fixtures.rs (env/helpers), mocks.rs (fake clients), assert.rs (test assertions).

pub mod assert;
pub mod fixtures;
pub mod mocks;
