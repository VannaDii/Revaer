//! Error types for documentation indexing operations.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

/// Result alias for doc indexer operations.
pub type Result<T> = std::result::Result<T, DocIndexError>;

/// Errors raised while building documentation manifests.
#[derive(Debug)]
pub enum DocIndexError {
    /// The docs root does not exist on disk.
    DocsRootMissing {
        /// Missing docs root path.
        path: PathBuf,
    },
    /// Failed to read a markdown file.
    ReadMarkdown {
        /// Markdown file path.
        path: PathBuf,
        /// IO error.
        source: std::io::Error,
    },
    /// Failed to parse markdown.
    ParseMarkdown {
        /// Markdown file path.
        path: PathBuf,
        /// Underlying parse error.
        source: Box<DocIndexError>,
    },
    /// A regex failed to compile.
    RegexCompile {
        /// Regex pattern.
        pattern: &'static str,
        /// Underlying regex error.
        source: regex::Error,
    },
    /// The summary was too short to be useful.
    SummaryTooShort {
        /// Markdown file path.
        path: PathBuf,
        /// Summary length in chars.
        chars: usize,
    },
    /// The docs path could not be made relative to the root.
    PathOutsideDocsRoot {
        /// Markdown file path.
        path: PathBuf,
        /// Docs root path.
        root: PathBuf,
    },
    /// A fallback summary could not be derived.
    FallbackSummaryMissing,
    /// Failed to read the schema file.
    SchemaRead {
        /// Schema path.
        path: PathBuf,
        /// IO error.
        source: std::io::Error,
    },
    /// Failed to parse the schema JSON.
    SchemaParse {
        /// Schema path.
        path: PathBuf,
        /// JSON parse error.
        source: serde_json::Error,
    },
    /// Failed to compile the schema validator.
    SchemaCompile {
        /// Validation error message.
        detail: String,
    },
    /// The manifest did not satisfy the schema.
    ManifestInvalid {
        /// Validation error message.
        detail: String,
    },
    /// Failed to serialise a JSON payload.
    SerializeJson {
        /// Payload label.
        label: &'static str,
        /// JSON error.
        source: serde_json::Error,
    },
    /// Failed to create an output directory.
    CreateDir {
        /// Directory path.
        path: PathBuf,
        /// IO error.
        source: std::io::Error,
    },
    /// Failed to write a JSON output file.
    WriteOutput {
        /// Output path.
        path: PathBuf,
        /// IO error.
        source: std::io::Error,
    },
}

impl Display for DocIndexError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DocsRootMissing { .. } => formatter.write_str("docs root missing"),
            Self::ReadMarkdown { .. } => formatter.write_str("failed to read markdown file"),
            Self::ParseMarkdown { .. } => formatter.write_str("failed to parse markdown"),
            Self::RegexCompile { .. } => formatter.write_str("failed to compile regex"),
            Self::SummaryTooShort { .. } => formatter.write_str("summary too short"),
            Self::PathOutsideDocsRoot { .. } => {
                formatter.write_str("markdown path outside docs root")
            }
            Self::FallbackSummaryMissing => {
                formatter.write_str("unable to derive fallback summary")
            }
            Self::SchemaRead { .. } => formatter.write_str("failed to read schema"),
            Self::SchemaParse { .. } => formatter.write_str("failed to parse schema json"),
            Self::SchemaCompile { .. } => formatter.write_str("schema compilation failed"),
            Self::ManifestInvalid { .. } => {
                formatter.write_str("manifest failed schema validation")
            }
            Self::SerializeJson { .. } => formatter.write_str("failed to serialise json payload"),
            Self::CreateDir { .. } => formatter.write_str("failed to create output directory"),
            Self::WriteOutput { .. } => formatter.write_str("failed to write output file"),
        }
    }
}

impl Error for DocIndexError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Self::ReadMarkdown { source, .. } = self {
            return Some(source);
        }
        if let Self::ParseMarkdown { source, .. } = self {
            return Some(source);
        }
        if let Self::RegexCompile { source, .. } = self {
            return Some(source);
        }
        if let Self::SchemaRead { source, .. } = self {
            return Some(source);
        }
        if let Self::SchemaParse { source, .. } = self {
            return Some(source);
        }
        if let Self::SerializeJson { source, .. } = self {
            return Some(source);
        }
        if let Self::CreateDir { source, .. } = self {
            return Some(source);
        }
        if let Self::WriteOutput { source, .. } = self {
            return Some(source);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::Error as _;
    use std::io;

    fn io_error() -> io::Error {
        io::Error::other("io")
    }

    fn json_error() -> serde_json::Error {
        match serde_json::from_str::<serde_json::Value>("invalid") {
            Ok(_) => serde_json::Error::custom("expected invalid json"),
            Err(err) => err,
        }
    }

    fn assert_error(err: &DocIndexError, message: &str, has_source: bool) {
        assert_eq!(err.to_string(), message);
        assert_eq!(err.source().is_some(), has_source);
    }

    #[test]
    fn doc_index_error_display_without_source() {
        assert_error(
            &DocIndexError::DocsRootMissing {
                path: PathBuf::from("docs"),
            },
            "docs root missing",
            false,
        );
        assert_error(
            &DocIndexError::SummaryTooShort {
                path: PathBuf::from("docs/readme.md"),
                chars: 10,
            },
            "summary too short",
            false,
        );
        assert_error(
            &DocIndexError::PathOutsideDocsRoot {
                path: PathBuf::from("docs/readme.md"),
                root: PathBuf::from("docs"),
            },
            "markdown path outside docs root",
            false,
        );
        assert_error(
            &DocIndexError::FallbackSummaryMissing,
            "unable to derive fallback summary",
            false,
        );
        assert_error(
            &DocIndexError::SchemaCompile {
                detail: "bad schema".to_string(),
            },
            "schema compilation failed",
            false,
        );
        assert_error(
            &DocIndexError::ManifestInvalid {
                detail: "bad manifest".to_string(),
            },
            "manifest failed schema validation",
            false,
        );
    }

    #[test]
    fn doc_index_error_display_with_source() {
        let parse_source = DocIndexError::ReadMarkdown {
            path: PathBuf::from("docs/readme.md"),
            source: io_error(),
        };
        let regex_error = regex::Error::Syntax("bad regex".to_string());

        assert_error(
            &DocIndexError::ReadMarkdown {
                path: PathBuf::from("docs/readme.md"),
                source: io_error(),
            },
            "failed to read markdown file",
            true,
        );
        assert_error(
            &DocIndexError::ParseMarkdown {
                path: PathBuf::from("docs/readme.md"),
                source: Box::new(parse_source),
            },
            "failed to parse markdown",
            true,
        );
        assert_error(
            &DocIndexError::RegexCompile {
                pattern: "[",
                source: regex_error,
            },
            "failed to compile regex",
            true,
        );
        assert_error(
            &DocIndexError::SchemaRead {
                path: PathBuf::from("schema.json"),
                source: io_error(),
            },
            "failed to read schema",
            true,
        );
        assert_error(
            &DocIndexError::SchemaParse {
                path: PathBuf::from("schema.json"),
                source: json_error(),
            },
            "failed to parse schema json",
            true,
        );
        assert_error(
            &DocIndexError::SerializeJson {
                label: "manifest",
                source: json_error(),
            },
            "failed to serialise json payload",
            true,
        );
        assert_error(
            &DocIndexError::CreateDir {
                path: PathBuf::from("dist"),
                source: io_error(),
            },
            "failed to create output directory",
            true,
        );
        assert_error(
            &DocIndexError::WriteOutput {
                path: PathBuf::from("dist/manifest.json"),
                source: io_error(),
            },
            "failed to write output file",
            true,
        );
    }
}
