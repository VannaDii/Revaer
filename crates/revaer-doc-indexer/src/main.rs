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
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]

//! Thin CLI entrypoint that delegates to the library implementation.

use std::path::PathBuf;

use revaer_doc_indexer::error::Result;
use revaer_doc_indexer::run;

/// Entry point for generating the documentation manifest and summaries files.
fn main() -> Result<()> {
    run_with_args(std::env::args())
}

fn run_with_args<I>(args: I) -> Result<()>
where
    I: IntoIterator<Item = String>,
{
    let mut iter = args.into_iter();
    let _ = iter.next();
    let docs_root = iter
        .next()
        .map_or_else(|| PathBuf::from("docs"), PathBuf::from);

    let schema_path = docs_root.join("llm").join("schema.json");
    run(&docs_root, &schema_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use revaer_doc_indexer::error::DocIndexError;

    #[test]
    fn run_with_args_surfaces_schema_errors() {
        let temp = std::env::temp_dir().join("revaer-doc-indexer-test");
        let _ = std::fs::create_dir_all(&temp);
        let args = vec![
            "revaer-doc-indexer".to_string(),
            temp.to_string_lossy().to_string(),
        ];
        let err = run_with_args(args).expect_err("expected schema failure");
        assert!(matches!(err, DocIndexError::SchemaRead { .. }));
        let _ = std::fs::remove_dir_all(&temp);
    }
}
