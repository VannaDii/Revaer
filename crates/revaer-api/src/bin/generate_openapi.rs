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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
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

    #[test]
    fn main_writes_openapi_document() -> Result<()> {
        let temp_root = env::temp_dir().join(format!("revaer-openapi-test-{}", Uuid::new_v4()));
        let docs_dir = temp_root.join("docs/api");
        fs::create_dir_all(&docs_dir).context("failed to prepare docs directory")?;
        let guard = WorkingDirGuard::change_to(temp_root.as_path())?;

        super::main().context("expected openapi export to succeed")?;

        drop(guard);
        let openapi_path = docs_dir.join("openapi.json");
        let payload = fs::read_to_string(&openapi_path)
            .with_context(|| format!("failed to read {}", openapi_path.display()))?;
        assert!(
            payload.contains("\"openapi\""),
            "generated document should contain openapi metadata"
        );
        fs::remove_dir_all(&temp_root).context("failed to clean up temporary directory")?;

        Ok(())
    }
}
