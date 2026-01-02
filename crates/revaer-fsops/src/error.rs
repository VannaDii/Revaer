//! # Design
//!
//! - Provide structured, constant-message errors for the fsops pipeline.
//! - Capture operation context (paths, fields, inputs) to make failures reproducible in tests.
//! - Preserve source errors without interpolating context into error messages.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Result type for filesystem operations.
pub type FsOpsResult<T> = Result<T, FsOpsError>;

/// Errors produced by filesystem post-processing.
#[derive(Debug, Error)]
pub enum FsOpsError {
    /// IO failures while interacting with the filesystem.
    #[error("fsops io failure")]
    Io {
        /// Operation that triggered the IO failure.
        operation: &'static str,
        /// Path involved in the IO failure.
        path: PathBuf,
        /// Underlying IO error.
        source: io::Error,
    },
    /// JSON parsing or serialization failures for metadata.
    #[error("fsops json failure")]
    Json {
        /// Operation that triggered the JSON failure.
        operation: &'static str,
        /// Path involved in the JSON failure.
        path: PathBuf,
        /// Underlying JSON error.
        source: serde_json::Error,
    },
    /// Walkdir traversal failures.
    #[error("fsops walkdir failure")]
    Walkdir {
        /// Operation that triggered the walkdir failure.
        operation: &'static str,
        /// Path involved in the walkdir failure.
        path: PathBuf,
        /// Underlying walkdir error.
        source: walkdir::Error,
    },
    /// Zip archive failures.
    #[error("fsops zip failure")]
    Zip {
        /// Operation that triggered the archive failure.
        operation: &'static str,
        /// Path involved in the archive failure.
        path: PathBuf,
        /// Underlying zip error.
        source: zip::result::ZipError,
    },
    /// Globset compilation failures.
    #[error("fsops glob failure")]
    Glob {
        /// Operation that triggered the glob failure.
        operation: &'static str,
        /// Glob pattern that failed to compile.
        pattern: String,
        /// Underlying globset error.
        source: globset::Error,
    },
    /// Policy validation failures.
    #[error("fsops invalid policy")]
    InvalidPolicy {
        /// Field that failed validation.
        field: &'static str,
        /// Static reason for the failure.
        reason: &'static str,
        /// Offending value when available.
        value: Option<String>,
    },
    /// Input validation failures.
    #[error("fsops invalid input")]
    InvalidInput {
        /// Field that failed validation.
        field: &'static str,
        /// Static reason for the failure.
        reason: &'static str,
        /// Offending value when available.
        value: Option<String>,
    },
    /// Unsupported operation or mode.
    #[error("fsops unsupported operation")]
    Unsupported {
        /// Operation that is unsupported.
        operation: &'static str,
        /// Optional value that triggered the unsupported error.
        value: Option<String>,
    },
    /// Required state was missing from the pipeline.
    #[error("fsops missing state")]
    MissingState {
        /// State field that was missing.
        field: &'static str,
    },
    /// User lookup failed when applying ownership changes.
    #[error("fsops user lookup failed")]
    UserLookup {
        /// Username that failed lookup.
        user: String,
        /// Underlying nix error.
        source: nix::Error,
    },
    /// Group lookup failed when applying ownership changes.
    #[error("fsops group lookup failed")]
    GroupLookup {
        /// Group name that failed lookup.
        group: String,
        /// Underlying nix error.
        source: nix::Error,
    },
    /// Nix syscall failures.
    #[error("fsops nix failure")]
    Nix {
        /// Operation that triggered the nix failure.
        operation: &'static str,
        /// Path involved in the nix failure.
        path: PathBuf,
        /// Underlying nix error.
        source: nix::Error,
    },
}

impl FsOpsError {
    pub(crate) fn io(operation: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            source,
        }
    }

    pub(crate) fn json(
        operation: &'static str,
        path: impl Into<PathBuf>,
        source: serde_json::Error,
    ) -> Self {
        Self::Json {
            operation,
            path: path.into(),
            source,
        }
    }

    pub(crate) fn walkdir(
        operation: &'static str,
        path: impl Into<PathBuf>,
        source: walkdir::Error,
    ) -> Self {
        Self::Walkdir {
            operation,
            path: path.into(),
            source,
        }
    }

    pub(crate) fn zip(
        operation: &'static str,
        path: impl Into<PathBuf>,
        source: zip::result::ZipError,
    ) -> Self {
        Self::Zip {
            operation,
            path: path.into(),
            source,
        }
    }

    pub(crate) const fn glob(
        operation: &'static str,
        pattern: String,
        source: globset::Error,
    ) -> Self {
        Self::Glob {
            operation,
            pattern,
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::Error as _;
    use std::error::Error;
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use walkdir::WalkDir;

    fn repo_root() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for ancestor in manifest_dir.ancestors() {
            if ancestor.join("AGENT.md").is_file() {
                return ancestor.to_path_buf();
            }
        }
        manifest_dir
    }

    fn server_root() -> Result<PathBuf, Box<dyn Error>> {
        let root = repo_root().join(".server_root");
        fs::create_dir_all(&root)?;
        Ok(root)
    }

    fn temp_dir() -> Result<TempDir, Box<dyn Error>> {
        Ok(tempfile::Builder::new()
            .prefix("revaer-fsops-")
            .tempdir_in(server_root()?)?)
    }

    fn io_error() -> io::Error {
        io::Error::other("io")
    }

    fn json_error() -> serde_json::Error {
        match serde_json::from_str::<serde_json::Value>("invalid") {
            Ok(_) => serde_json::Error::custom("expected invalid json"),
            Err(err) => err,
        }
    }

    #[test]
    fn fsops_error_helpers_build_variants() -> Result<(), Box<dyn Error>> {
        let io_err = FsOpsError::io("read", "path", io_error());
        assert!(matches!(io_err, FsOpsError::Io { .. }));
        assert!(io_err.source().is_some());

        let json_err = FsOpsError::json("parse", "path", json_error());
        assert!(matches!(json_err, FsOpsError::Json { .. }));
        assert!(json_err.source().is_some());

        let temp = temp_dir()?;
        let missing = temp.path().join("missing");
        let walkdir_error = WalkDir::new(&missing)
            .into_iter()
            .next()
            .and_then(Result::err)
            .ok_or_else(|| io::Error::other("expected walkdir error"))?;
        let walk_err = FsOpsError::walkdir("walk", &missing, walkdir_error);
        assert!(matches!(walk_err, FsOpsError::Walkdir { .. }));
        assert!(walk_err.source().is_some());

        let zip_err = FsOpsError::zip("unpack", "archive.zip", zip::result::ZipError::FileNotFound);
        assert!(matches!(zip_err, FsOpsError::Zip { .. }));
        assert!(zip_err.source().is_some());

        let Err(glob_error) = globset::Glob::new("[") else {
            return Err(io::Error::other("expected glob error").into());
        };
        let glob_err = FsOpsError::glob("compile", "[".to_string(), glob_error);
        assert!(matches!(glob_err, FsOpsError::Glob { .. }));
        assert!(glob_err.source().is_some());
        Ok(())
    }
}
