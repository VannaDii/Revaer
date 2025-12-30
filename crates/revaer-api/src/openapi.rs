//! `OpenAPI` document helpers and dependency wiring.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use tracing::error;

use crate::openapi_assets::OPENAPI_EMBEDDED_JSON;

type OpenApiPersistFn =
    Arc<dyn Fn(&Path, &Value) -> Result<(), revaer_telemetry::TelemetryError> + Send + Sync>;

pub(crate) struct OpenApiDependencies {
    pub(crate) document: Arc<Value>,
    pub(crate) path: PathBuf,
    pub(crate) persist: OpenApiPersistFn,
}

impl OpenApiDependencies {
    pub(crate) fn new(document: Arc<Value>, path: PathBuf, persist: OpenApiPersistFn) -> Self {
        Self {
            document,
            path,
            persist,
        }
    }

    pub(crate) fn embedded_at(path: &Path) -> Self {
        Self::new(
            Arc::new(build_openapi_document()),
            path.to_path_buf(),
            Arc::new(|destination, document| {
                revaer_telemetry::persist_openapi(destination, document)?;
                Ok(())
            }),
        )
    }
}

pub(crate) fn build_openapi_document() -> Value {
    match serde_json::from_str(OPENAPI_EMBEDDED_JSON) {
        Ok(value) => value,
        Err(err) => {
            error!(error = %err, "failed to parse embedded OpenAPI document");
            Value::Object(serde_json::Map::new())
        }
    }
}

#[must_use]
/// Return a fresh copy of the embedded `OpenAPI` specification.
pub fn openapi_document() -> Value {
    build_openapi_document()
}

#[must_use]
/// Return the default `OpenAPI` output path.
pub fn openapi_output_path() -> PathBuf {
    crate::openapi_assets::openapi_output_path()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openapi_assets::OPENAPI_FILENAME;
    use serde_json::json;
    use std::fs;
    use std::io;
    use uuid::Uuid;

    #[test]
    fn build_openapi_document_parses_embedded_json() {
        let document = build_openapi_document();
        assert!(
            document.is_object(),
            "embedded OpenAPI document should decode to a JSON object"
        );
    }

    #[test]
    fn openapi_document_returns_fresh_instance() -> Result<(), Box<dyn std::error::Error>> {
        let a = openapi_document();
        let mut b = openapi_document();
        b.as_object_mut()
            .ok_or_else(|| io::Error::other("expected object"))?
            .insert("x".into(), json!(1));
        assert!(a.get("x").is_none(), "documents are independent");
        Ok(())
    }

    #[test]
    fn embedded_dependencies_invoke_persist_hook() -> Result<(), Box<dyn std::error::Error>> {
        let document = Arc::new(json!({"openapi": "3.0.0"}));
        let dir = std::env::temp_dir().join(format!("openapi-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let dest = dir.join(OPENAPI_FILENAME);
        let invoked = Arc::new(std::sync::Mutex::new(Vec::new()));
        let persist = {
            let record = Arc::clone(&invoked);
            Arc::new(move |path: &Path, value: &Value| {
                record
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(path.to_path_buf());
                assert_eq!(value["openapi"], "3.0.0");
                Ok(())
            }) as OpenApiPersistFn
        };

        let deps = OpenApiDependencies::new(document, dest.clone(), persist);
        (deps.persist)(&dest, &deps.document)?;

        let paths = invoked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(paths.as_slice(), &[dest]);
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
