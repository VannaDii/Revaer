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
#![allow(clippy::module_name_repetitions)]
#![allow(unexpected_cfgs)]

//! Helper binary that exports the generated `OpenAPI` document to `docs/api/`.

use std::path::Path;

use anyhow::Result;

/// Serialises the generated `OpenAPI` document to `docs/api/openapi.json`.
fn main() -> Result<()> {
    let document = revaer_api::openapi_document();
    revaer_telemetry::persist_openapi(Path::new("docs/api/openapi.json"), &document)?;
    println!("OpenAPI document written to docs/api/openapi.json");
    Ok(())
}
