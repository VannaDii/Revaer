//! `OpenAPI` document helpers and dependency wiring.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;

type OpenApiPersistFn = Arc<dyn Fn(&Path, &Value) -> Result<()> + Send + Sync>;

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
    match serde_json::from_str(include_str!("../../../docs/api/openapi.json")) {
        Ok(value) => value,
        Err(err) => panic!("embedded OpenAPI document is invalid JSON: {err}"),
    }
}

#[must_use]
/// Return a fresh copy of the embedded `OpenAPI` specification.
pub fn openapi_document() -> Value {
    build_openapi_document()
}
