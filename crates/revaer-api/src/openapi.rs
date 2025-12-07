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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
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
    fn openapi_document_returns_fresh_instance() {
        let a = openapi_document();
        let mut b = openapi_document();
        b.as_object_mut()
            .expect("object")
            .insert("x".into(), json!(1));
        assert!(a.get("x").is_none(), "documents are independent");
    }

    #[test]
    fn embedded_dependencies_invoke_persist_hook() {
        let document = Arc::new(json!({"openapi": "3.0.0"}));
        let dir = std::env::temp_dir().join(format!("openapi-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("tempdir");
        let dest = dir.join("openapi.json");
        let invoked = Arc::new(std::sync::Mutex::new(Vec::new()));
        let persist = {
            let record = Arc::clone(&invoked);
            Arc::new(move |path: &Path, value: &Value| {
                record.lock().unwrap().push(path.to_path_buf());
                assert_eq!(value["openapi"], "3.0.0");
                Ok(())
            }) as OpenApiPersistFn
        };

        let deps = OpenApiDependencies::new(document, dest.clone(), persist);
        (deps.persist)(&dest, &deps.document).expect("persist hook");

        let paths = {
            let guard = invoked.lock().unwrap();
            guard.clone()
        };
        assert_eq!(paths.as_slice(), &[dest]);
        let _ = fs::remove_dir_all(&dir);
    }
}
