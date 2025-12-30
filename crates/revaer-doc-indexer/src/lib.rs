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
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]

//! Documentation indexer core: parses markdown under `docs/` and emits machine-readable
//! manifests consumed by LLM tooling.
//!
//! # Design
//! - Pure library surface (`run`) used by the thin CLI entrypoint.
//! - No IO outside the provided docs root; callers supply paths for determinism.
//! - Markdown parsing is kept minimal and deterministic (no HTML).

use crate::error::{DocIndexError, Result};
use chrono::Utc;
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
#[cfg(test)]
use std::path::PathBuf;
use std::path::{Component, Path};
use walkdir::WalkDir;

pub mod error;

/// Manifest entry for a single markdown page.
#[derive(Serialize, Clone)]
pub struct Entry {
    /// Stable identifier derived from the docs path (without `.md`).
    pub id: String,
    /// Primary page title (first `# ` heading).
    pub title: String,
    /// Short summary captured from blockquote or derived fallback.
    pub summary: String,
    /// Normalised heading tags derived from `##` lines.
    pub tags: Vec<String>,
    /// Link to the rendered page.
    pub href: String,
}

#[derive(Serialize)]
struct Manifest {
    version: String,
    generated: String,
    entries: Vec<Entry>,
}

/// Generate documentation manifests for the given docs tree and schema.
///
/// # Errors
///
/// Returns an error if the docs root is missing, markdown cannot be parsed,
/// or the manifest fails schema validation/writing.
pub fn run(docs_root: &Path, schema_path: &Path) -> Result<()> {
    if !docs_root.exists() {
        return Err(DocIndexError::DocsRootMissing {
            path: docs_root.to_path_buf(),
        });
    }

    let mut entries = Vec::new();
    for entry in WalkDir::new(docs_root)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        if should_skip(path, docs_root) {
            continue;
        }

        let raw = fs::read_to_string(path).map_err(|source| DocIndexError::ReadMarkdown {
            path: path.to_path_buf(),
            source,
        })?;

        let maybe_page = parse_markdown(&raw, path, docs_root).map_err(|source| {
            DocIndexError::ParseMarkdown {
                path: path.to_path_buf(),
                source: Box::new(source),
            }
        })?;

        if let Some(page) = maybe_page {
            entries.push(page);
        }
    }

    entries.sort_by(|a, b| a.id.cmp(&b.id));

    let manifest = Manifest {
        version: env!("CARGO_PKG_VERSION").to_string(),
        generated: Utc::now().to_rfc3339(),
        entries: entries.clone(),
    };

    validate_manifest(&manifest, schema_path)?;

    let llm_dir = docs_root.join("llm");
    fs::create_dir_all(&llm_dir).map_err(|source| DocIndexError::CreateDir {
        path: llm_dir,
        source,
    })?;

    let manifest_path = docs_root.join("llm/manifest.json");
    let summaries_path = docs_root.join("llm/summaries.json");

    let manifest_json =
        serde_json::to_string_pretty(&manifest).map_err(|source| DocIndexError::SerializeJson {
            label: "manifest",
            source,
        })?;
    fs::write(&manifest_path, manifest_json).map_err(|source| DocIndexError::WriteOutput {
        path: manifest_path.clone(),
        source,
    })?;

    let summaries_json =
        serde_json::to_string_pretty(&entries).map_err(|source| DocIndexError::SerializeJson {
            label: "summaries",
            source,
        })?;
    fs::write(&summaries_path, summaries_json).map_err(|source| DocIndexError::WriteOutput {
        path: summaries_path.clone(),
        source,
    })?;

    println!(
        "Generated {} entries → {}",
        manifest.entries.len(),
        manifest_path.display()
    );

    Ok(())
}

fn should_skip(path: &Path, docs_root: &Path) -> bool {
    if path.file_name().and_then(|s| s.to_str()) == Some("SUMMARY.md") {
        return true;
    }

    if let Ok(relative) = path.strip_prefix(docs_root) {
        for component in relative.components() {
            if matches!(component, Component::Normal(name) if name.to_string_lossy().starts_with('_'))
            {
                return true;
            }
        }
    }

    false
}

fn parse_markdown(raw: &str, path: &Path, docs_root: &Path) -> Result<Option<Entry>> {
    let mut title: Option<String> = None;
    let mut summary_lines: Vec<String> = Vec::new();
    let mut headings: Vec<String> = Vec::new();
    let mut capture_summary = false;
    let h1_regex = Regex::new(r"^# (.+)$").map_err(|source| DocIndexError::RegexCompile {
        pattern: "^# (.+)$",
        source,
    })?;
    let h2_regex = Regex::new(r"^## (.+)$").map_err(|source| DocIndexError::RegexCompile {
        pattern: "^## (.+)$",
        source,
    })?;

    for line in raw.lines() {
        if title.is_none() {
            if let Some(caps) = h1_regex.captures(line.trim()) {
                title = Some(caps[1].trim().to_string());
            }
            continue;
        }

        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            // Stop capturing summary/tag lines inside code blocks.
            capture_summary = false;
        }

        if trimmed.starts_with('>') && summary_lines.is_empty() {
            capture_summary = true;
            summary_lines.push(trimmed.trim_start_matches('>').trim().to_string());
            continue;
        } else if capture_summary && trimmed.starts_with('>') {
            summary_lines.push(trimmed.trim_start_matches('>').trim().to_string());
            continue;
        } else if capture_summary && !trimmed.starts_with('>') {
            capture_summary = false;
        }

        if let Some(caps) = h2_regex.captures(trimmed) {
            headings.push(caps[1].trim().to_string());
        }
    }

    let Some(title) = title else {
        return Ok(None);
    };

    let summary = if summary_lines.is_empty() {
        fallback_summary(raw)?
    } else {
        summary_lines.join(" ")
    };

    if summary.chars().count() < 10 {
        return Err(DocIndexError::SummaryTooShort {
            path: path.to_path_buf(),
            chars: summary.chars().count(),
        });
    }

    let relative =
        path.strip_prefix(docs_root)
            .map_err(|_| DocIndexError::PathOutsideDocsRoot {
                path: path.to_path_buf(),
                root: docs_root.to_path_buf(),
            })?;
    let mut id = relative
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    if let Some(stripped) = id.strip_suffix(".md") {
        id = stripped.to_string();
    }

    let href = format!("/{id}/");
    let tags = normalise_tags(&headings);

    Ok(Some(Entry {
        id,
        title,
        summary,
        tags,
        href,
    }))
}

fn fallback_summary(raw: &str) -> Result<String> {
    let mut text = String::new();
    let mut seen_h1 = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("# ") {
            seen_h1 = true;
            continue;
        }

        if !seen_h1 {
            continue;
        }

        if trimmed.starts_with("## ") {
            if !text.is_empty() {
                break;
            }
            continue;
        }

        if trimmed.starts_with('>') {
            if text.is_empty() {
                text.push_str(trimmed.trim_start_matches('>').trim());
            }
            continue;
        }

        if trimmed.starts_with("```") {
            continue;
        }

        let mut content = trimmed;
        if let Some(stripped) = content.strip_prefix("- ") {
            content = stripped;
        } else if let Some(stripped) = content.strip_prefix("* ") {
            content = stripped;
        } else if let Some((digits, rest)) = content.split_once(". ")
            && digits.chars().all(|ch| ch.is_ascii_digit())
        {
            content = rest.trim_start();
        } else if let Some((digits, rest)) = content.split_once(") ")
            && digits.chars().all(|ch| ch.is_ascii_digit())
        {
            content = rest.trim_start();
        }

        if content.is_empty() {
            continue;
        }

        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(content);
    }

    let cleaned = text.replace('`', "");
    let limited = cleaned
        .split_whitespace()
        .take(100)
        .collect::<Vec<_>>()
        .join(" ");

    if limited.is_empty() {
        return Err(DocIndexError::FallbackSummaryMissing);
    }

    Ok(limited)
}

fn normalise_tags(headings: &[String]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for heading in headings {
        let slug = slugify(heading);
        if !slug.is_empty() {
            set.insert(slug);
        }
    }
    set.into_iter().collect()
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if matches!(ch, ' ' | '-' | '_' | '/') {
            if !prev_dash {
                slug.push('-');
                prev_dash = true;
            }
        } else {
            // drop other characters
        }
    }
    if slug.ends_with('-') {
        slug.pop();
    }
    slug
}

fn validate_manifest(manifest: &Manifest, schema_path: &Path) -> Result<()> {
    let schema_str =
        fs::read_to_string(schema_path).map_err(|source| DocIndexError::SchemaRead {
            path: schema_path.to_path_buf(),
            source,
        })?;

    let schema_value: serde_json::Value =
        serde_json::from_str(&schema_str).map_err(|source| DocIndexError::SchemaParse {
            path: schema_path.to_path_buf(),
            source,
        })?;
    let schema_ref: &'static serde_json::Value = Box::leak(Box::new(schema_value));
    let compiled =
        jsonschema::Validator::new(schema_ref).map_err(|err| DocIndexError::SchemaCompile {
            detail: err.to_string(),
        })?;
    let manifest_value =
        serde_json::to_value(manifest).map_err(|source| DocIndexError::SerializeJson {
            label: "manifest_validate",
            source,
        })?;

    if let Err(error) = compiled.validate(&manifest_value) {
        return Err(DocIndexError::ManifestInvalid {
            detail: error.to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::env;
    use std::error::Error;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> std::result::Result<Self, Box<dyn Error>> {
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let mut root = env::temp_dir();
            root.push(format!("revaer-doc-indexer-{nanos}-{}", std::process::id()));
            fs::create_dir_all(&root)?;
            Ok(Self { path: root })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_file(path: &Path, contents: &str) -> std::result::Result<(), Box<dyn Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(path)?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }

    #[test]
    fn run_generates_manifest_and_summaries() -> std::result::Result<(), Box<dyn Error>> {
        let temp = TempDir::new()?;
        let docs_root = temp.path().join("docs");
        let schema_path = docs_root.join("llm/schema.json");

        let schema = r#"
        {
            "type": "object",
            "required": ["version", "generated", "entries"],
            "properties": {
                "version": { "type": "string" },
                "generated": { "type": "string" },
                "entries": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["id", "title", "summary", "tags", "href"],
                        "properties": {
                            "id": { "type": "string" },
                            "title": { "type": "string" },
                            "summary": { "type": "string", "minLength": 10 },
                            "tags": {
                                "type": "array",
                                "items": { "type": "string" }
                            },
                            "href": { "type": "string" }
                        }
                    }
                }
            }
        }"#;

        write_file(&schema_path, schema)?;

        let markdown = "# Getting Started
> Welcome to Revaer
> Build resilient systems.

## Features
## Usage

- Bullet one
- Bullet two
";
        write_file(&docs_root.join("guide.md"), markdown)?;

        // Confirm skip logic ignores underscores and SUMMARY.md.
        write_file(&docs_root.join("_drafts/draft.md"), "# Draft\n")?;
        write_file(&docs_root.join("SUMMARY.md"), "# Summary\n")?;

        run(&docs_root, &schema_path)?;

        let manifest_path = docs_root.join("llm/manifest.json");
        let summaries_path = docs_root.join("llm/summaries.json");

        assert!(manifest_path.exists(), "manifest.json missing");
        assert!(summaries_path.exists(), "summaries.json missing");

        let manifest_json = fs::read_to_string(&manifest_path)?;
        let manifest: Value = serde_json::from_str(&manifest_json)?;
        let entries = manifest["entries"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("entries array missing"))?;
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry["id"], "guide");
        assert_eq!(entry["title"], "Getting Started");
        assert_eq!(
            entry["summary"],
            "Welcome to Revaer Build resilient systems."
        );
        assert_eq!(entry["href"], "/guide/");
        let tags = entry["tags"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("tags array missing"))?
            .iter()
            .map(|value| value.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| std::io::Error::other("tag value missing"))?;
        assert_eq!(tags, vec!["features", "usage"]);

        let summaries_json = fs::read_to_string(&summaries_path)?;
        let summaries: Value = serde_json::from_str(&summaries_json)?;
        let summaries = summaries
            .as_array()
            .ok_or_else(|| std::io::Error::other("summaries array missing"))?;
        assert_eq!(summaries.len(), 1);

        Ok(())
    }

    #[test]
    fn run_fails_for_missing_docs_root() -> std::result::Result<(), Box<dyn Error>> {
        let missing = PathBuf::from("target/non-existent-docs");
        let error = run(&missing, &missing.join("llm/schema.json"))
            .err()
            .ok_or_else(|| std::io::Error::other("missing docs root should error"))?;
        assert!(matches!(error, DocIndexError::DocsRootMissing { .. }));
        Ok(())
    }

    #[test]
    fn parse_markdown_extracts_title_summary_and_tags() -> std::result::Result<(), Box<dyn Error>> {
        let docs_root = PathBuf::from("docs");
        let path = docs_root.join("subdir/page.md");
        let markdown = "# Example Page
> Concise summary line.

## First Heading
## Second Heading

Additional text.
";

        let entry = parse_markdown(markdown, &path, &docs_root)?
            .ok_or_else(|| std::io::Error::other("entry missing"))?;
        assert_eq!(entry.id, "subdir/page");
        assert_eq!(entry.title, "Example Page");
        assert_eq!(entry.summary, "Concise summary line.");
        assert_eq!(entry.href, "/subdir/page/");
        assert_eq!(
            entry.tags,
            vec!["first-heading".to_string(), "second-heading".to_string()]
        );
        Ok(())
    }

    #[test]
    fn parse_markdown_respects_fallback_summary() -> std::result::Result<(), Box<dyn Error>> {
        let docs_root = PathBuf::from("docs");
        let path = docs_root.join("fallback.md");
        let markdown = "# Title

Paragraph one introduces the concept.
- Bullet A elaborates.

## Details
More text.
";
        let entry = parse_markdown(markdown, &path, &docs_root)?
            .ok_or_else(|| std::io::Error::other("fallback entry missing"))?;
        assert!(
            entry
                .summary
                .contains("Paragraph one introduces the concept."),
            "fallback summary missing expected text: {}",
            entry.summary
        );
        Ok(())
    }

    #[test]
    fn fallback_summary_rejects_empty_content() -> std::result::Result<(), Box<dyn Error>> {
        let err = fallback_summary("# Title\n\n## Heading\n")
            .err()
            .ok_or_else(|| std::io::Error::other("expected failure"))?;
        assert!(matches!(err, DocIndexError::FallbackSummaryMissing));
        Ok(())
    }

    #[test]
    fn should_skip_filters_summary_and_hidden_dirs() {
        let root = PathBuf::from("docs");
        assert!(should_skip(&root.join("SUMMARY.md"), &root));
        assert!(should_skip(&root.join("_hidden/page.md"), &root));
        assert!(!should_skip(&root.join("visible/page.md"), &root));
    }

    #[test]
    fn slugify_normalises_and_deduplicates_dashes() {
        assert_eq!(slugify("API Overview / HTTP"), "api-overview-http");
        assert_eq!(slugify("Trailing Dash-"), "trailing-dash");
        assert_eq!(slugify("Symbols! & Numbers 123"), "symbols-numbers-123");
    }

    #[test]
    fn normalise_tags_deduplicates_headings() {
        let headings = vec![
            "First Topic".to_string(),
            "Second Topic".to_string(),
            "First Topic".to_string(),
        ];
        let tags = normalise_tags(&headings);
        assert_eq!(
            tags,
            vec!["first-topic".to_string(), "second-topic".to_string()]
        );
    }

    #[test]
    fn validate_manifest_detects_schema_mismatch() -> std::result::Result<(), Box<dyn Error>> {
        let temp = TempDir::new()?;
        let schema_path = temp.path().join("schema.json");
        write_file(
            &schema_path,
            r#"{
                "type": "object",
                "required": ["version", "entries", "generated"],
                "properties": {
                    "version": { "type": "string", "pattern": "^v" },
                    "generated": { "type": "string" },
                    "entries": { "type": "array" }
                }
            }"#,
        )?;

        let manifest = Manifest {
            version: "0.1.0".into(),
            generated: "2024-01-01T00:00:00Z".into(),
            entries: Vec::new(),
        };

        let err = validate_manifest(&manifest, &schema_path)
            .err()
            .ok_or_else(|| std::io::Error::other("pattern mismatch should fail validation"))?;
        assert!(matches!(err, DocIndexError::ManifestInvalid { .. }));
        Ok(())
    }
}
