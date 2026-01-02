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

//! Helper binary that exports the generated `OpenAPI` document to `docs/api/`.

use anyhow::Result;
use serde_json::Value;
use std::path::Path;

/// Serialises the generated `OpenAPI` document to `docs/api/openapi.json`.
fn main() -> Result<()> {
    let document = revaer_api::openapi_document();
    let target = revaer_api::openapi_output_path();
    export_openapi(&document, &target, |path, doc| {
        revaer_telemetry::persist_openapi(path, doc)?;
        Ok(())
    })?;
    Ok(())
}

/// Persist the provided `OpenAPI` document using the supplied writer.
fn export_openapi(
    document: &Value,
    destination: &Path,
    persist: impl FnOnce(&Path, &Value) -> Result<()>,
) -> Result<()> {
    persist(destination, document)?;
    println!("OpenAPI document written to {}", destination.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use serde_json::json;
    use std::{
        env, fs,
        path::{Path, PathBuf},
    };
    use uuid::Uuid;

    struct WorkingDirGuard {
        original: PathBuf,
    }

    impl WorkingDirGuard {
        fn change_to(target: &Path) -> Result<Self> {
            let original = env::current_dir().context("failed to capture current directory")?;
            env::set_current_dir(target).context("failed to change working directory")?;
            Ok(Self { original })
        }
    }

    impl Drop for WorkingDirGuard {
        fn drop(&mut self) {
            let _ = env::set_current_dir(&self.original);
        }
    }

    fn repo_root() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for ancestor in manifest_dir.ancestors() {
            if ancestor.join("AGENT.md").is_file() {
                return ancestor.to_path_buf();
            }
        }
        manifest_dir
    }

    fn server_root() -> Result<PathBuf> {
        let root = repo_root().join(".server_root");
        fs::create_dir_all(&root).context("failed to create server root")?;
        Ok(root)
    }

    #[test]
    fn main_writes_openapi_document() -> Result<()> {
        let temp_root = server_root()?.join(format!("revaer-openapi-test-{}", Uuid::new_v4()));
        let docs_dir = temp_root.join("docs/api");
        fs::create_dir_all(&docs_dir).context("failed to prepare docs directory")?;
        let guard = WorkingDirGuard::change_to(temp_root.as_path())?;

        super::main().context("expected openapi export to succeed")?;

        drop(guard);
        let openapi_path = temp_root.join(revaer_api::openapi_output_path());
        let payload = fs::read_to_string(&openapi_path)
            .with_context(|| format!("failed to read {}", openapi_path.display()))?;
        assert!(
            payload.contains("\"openapi\""),
            "generated document should contain openapi metadata"
        );
        fs::remove_dir_all(&temp_root).context("failed to clean up temporary directory")?;

        Ok(())
    }

    #[test]
    fn export_openapi_uses_injected_persister() -> Result<()> {
        let document = json!({ "openapi": "3.1.0" });
        let target = revaer_api::openapi_output_path();
        let mut persisted = false;

        export_openapi(&document, &target, |path, payload| {
            assert_eq!(path, &target);
            assert_eq!(payload, &document);
            persisted = true;
            Ok(())
        })?;

        assert!(persisted, "export should invoke injected persister");

        Ok(())
    }
}
