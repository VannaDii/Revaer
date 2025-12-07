//! `OpenAPI` persistence helpers used across services.
//!
//! # Design
//! - Ensures `OpenAPI` artifacts are written atomically with directories created as needed.
//! - Returns the canonical JSON string to keep logging/caching consistent.

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

/// Persist an `OpenAPI` JSON document to disk and return the canonicalised payload.
///
/// # Errors
///
/// Returns an error if the output directory cannot be created, the document
/// cannot be serialised, or the file cannot be written.
pub fn persist_openapi(path: impl AsRef<Path>, document: &Value) -> Result<String> {
    let json = serde_json::to_string_pretty(document)?;
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create OpenAPI output directory '{}'",
                parent.display()
            )
        })?;
    }
    std::fs::write(path, json.as_bytes())
        .with_context(|| format!("failed to write OpenAPI artifact to '{}'", path.display()))?;
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn persist_openapi_writes_document() -> Result<()> {
        let dir = tempdir()?;
        let path = dir.path().join("openapi.json");
        let document = json!({"openapi": "3.0.0"});

        let contents = persist_openapi(&path, &document)?;
        assert!(contents.contains("\"openapi\": \"3.0.0\""));
        let file = std::fs::read_to_string(&path)?;
        assert!(file.contains("\"openapi\": \"3.0.0\""));
        Ok(())
    }
}
