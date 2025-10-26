#![allow(unexpected_cfgs)]

use std::path::Path;

use anyhow::Result;

fn main() -> Result<()> {
    let document = revaer_api::openapi_document();
    revaer_telemetry::persist_openapi(Path::new("docs/api/openapi.json"), &document)?;
    println!("OpenAPI document written to docs/api/openapi.json");
    Ok(())
}
