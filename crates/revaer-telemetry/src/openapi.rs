//! `OpenAPI` persistence helpers used across services.
//!
//! # Design
//! - Ensures `OpenAPI` artifacts are written atomically with directories created as needed.
//! - Returns the canonical JSON string to keep logging/caching consistent.

use std::path::Path;

use crate::error::{Result, TelemetryError};
use serde_json::Value;

/// Persist an `OpenAPI` JSON document to disk and return the canonicalised payload.
///
/// # Errors
///
/// Returns an error if the output directory cannot be created, the document
/// cannot be serialised, or the file cannot be written.
pub fn persist_openapi(path: impl AsRef<Path>, document: &Value) -> Result<String> {
    let json = serde_json::to_string_pretty(document)
        .map_err(|source| TelemetryError::OpenApiSerialize { source })?;
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| TelemetryError::OpenApiCreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, json.as_bytes()).map_err(|source| TelemetryError::OpenApiWrite {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn repo_root() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for ancestor in manifest_dir.ancestors() {
            if ancestor.join("AGENT.md").is_file() {
                return ancestor.to_path_buf();
            }
        }
        manifest_dir
    }

    fn server_root() -> std::result::Result<PathBuf, Box<dyn std::error::Error>> {
        let root = repo_root().join(".server_root");
        fs::create_dir_all(&root)?;
        Ok(root)
    }

    fn temp_dir() -> std::result::Result<TempDir, Box<dyn std::error::Error>> {
        Ok(tempfile::Builder::new()
            .prefix("revaer-telemetry-")
            .tempdir_in(server_root()?)?)
    }

    #[test]
    fn persist_openapi_writes_document() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = temp_dir()?;
        let path = dir.path().join("openapi.json");
        let document = json!({"openapi": "3.0.0"});

        let contents = persist_openapi(&path, &document)?;
        assert!(contents.contains("\"openapi\": \"3.0.0\""));
        let file = std::fs::read_to_string(&path)?;
        assert!(file.contains("\"openapi\": \"3.0.0\""));
        Ok(())
    }

    #[test]
    fn persist_openapi_reports_create_dir_failure()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = temp_dir()?;
        let file_path = dir.path().join("openapi-file");
        std::fs::write(&file_path, "not a dir")?;
        let path = file_path.join("openapi.json");
        let document = json!({"openapi": "3.0.0"});

        let Err(err) = persist_openapi(&path, &document) else {
            return Err(io::Error::other("expected create dir error").into());
        };
        assert!(matches!(err, TelemetryError::OpenApiCreateDir { .. }));
        Ok(())
    }

    #[test]
    fn persist_openapi_reports_write_failure() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        let dir = temp_dir()?;
        let document = json!({"openapi": "3.0.0"});

        let Err(err) = persist_openapi(dir.path(), &document) else {
            return Err(io::Error::other("expected write error").into());
        };
        assert!(matches!(err, TelemetryError::OpenApiWrite { .. }));
        Ok(())
    }
}
