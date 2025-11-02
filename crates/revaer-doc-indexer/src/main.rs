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
#![allow(clippy::multiple_crate_versions)]

//! Documentation indexer CLI that produces machine-readable manifests for the
//! Revaer docs tree.

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

#[derive(Serialize, Clone)]
struct Entry {
    id: String,
    title: String,
    summary: String,
    tags: Vec<String>,
    href: String,
}

#[derive(Serialize)]
struct Manifest {
    version: String,
    generated: String,
    entries: Vec<Entry>,
}

/// Entry point for generating the documentation manifest and summaries files.
fn main() -> Result<()> {
    let docs_root = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("docs"), PathBuf::from);

    let schema_path = docs_root.join("llm").join("schema.json");
    run(&docs_root, &schema_path)
}

fn run(docs_root: &Path, schema_path: &Path) -> Result<()> {
    if !docs_root.exists() {
        return Err(anyhow!("Docs path {} does not exist", docs_root.display()));
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

        let raw = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let maybe_page = parse_markdown(&raw, path, docs_root)
            .with_context(|| format!("Failed to parse markdown for {}", path.display()))?;

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

    fs::create_dir_all(docs_root.join("llm"))
        .with_context(|| format!("Failed to create {}", docs_root.join("llm").display()))?;

    let manifest_path = docs_root.join("llm/manifest.json");
    let summaries_path = docs_root.join("llm/summaries.json");

    let manifest_json =
        serde_json::to_string_pretty(&manifest).context("Failed to serialise manifest payload")?;
    fs::write(&manifest_path, manifest_json)
        .with_context(|| format!("Failed to write {}", manifest_path.display()))?;

    let summaries_json =
        serde_json::to_string_pretty(&entries).context("Failed to serialise summary payload")?;
    fs::write(&summaries_path, summaries_json)
        .with_context(|| format!("Failed to write {}", summaries_path.display()))?;

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
    let h1_regex = Regex::new(r"^# (.+)$").context("Failed to compile H1 regex")?;
    let h2_regex = Regex::new(r"^## (.+)$").context("Failed to compile H2 regex")?;

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
        return Err(anyhow!(
            "Summary too short for {} ({} chars)",
            path.display(),
            summary.chars().count()
        ));
    }

    let relative = path
        .strip_prefix(docs_root)
        .with_context(|| format!("{} lives outside docs root", path.display()))?;
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
        return Err(anyhow!("Unable to derive fallback summary"));
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
    let schema_str = fs::read_to_string(schema_path).with_context(|| {
        format!(
            "Failed to read JSON schema at {}. Run `just docs-index` after adding schema.json.",
            schema_path.display()
        )
    })?;

    let schema_value: serde_json::Value =
        serde_json::from_str(&schema_str).context("Invalid JSON schema format")?;
    let schema_ref: &'static serde_json::Value = Box::leak(Box::new(schema_value));
    let compiled = jsonschema::Validator::new(schema_ref).context("Schema compilation failed")?;
    let manifest_value =
        serde_json::to_value(manifest).context("Failed to serialise manifest for validation")?;

    if let Err(error) = compiled.validate(&manifest_value) {
        eprintln!("Schema validation error: {error}");
        return Err(anyhow!("Manifest failed schema validation"));
    }

    Ok(())
}
