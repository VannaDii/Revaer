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
//! Sync Nexus vendor assets into the Revaer UI static directory.
//!
//! # Design
//! - Resolves the UI root relative to `CARGO_MANIFEST_DIR` so it can be run from any cwd.
//! - Copies CSS, images, and JS into `static/nexus`, replacing any previous outputs.
//! - Validates the copied CSS for size and a `DaisyUI` marker before writing the lock file.
//! - Emits a deterministic `ASSET_LOCK.txt` containing the CSS hash and directory stats.
//!
//! Failure modes include missing vendor inputs, copy errors, invalid CSS contents,
//! or inability to write outputs and the lock file.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use fs_extra::dir::CopyOptions;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

const VENDOR_ROOT: &str = "ui_vendor/nexus-html@3.1.0";
const OUTPUT_ROOT: &str = "static/nexus";
const MIN_CSS_BYTES: usize = 1024;
const CSS_MARKER: &str = ".btn";

/// Errors returned by the asset sync tool.
#[derive(Debug)]
pub enum AssetSyncError {
    /// A required path is missing on disk.
    MissingPath {
        /// Path that could not be found.
        path: PathBuf,
    },
    /// A required file path is not a file.
    ExpectedFile {
        /// Path that was expected to be a file.
        path: PathBuf,
    },
    /// A required directory path is not a directory.
    ExpectedDir {
        /// Path that was expected to be a directory.
        path: PathBuf,
    },
    /// A filesystem operation failed.
    Io {
        /// Path involved in the failing IO operation.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A directory copy failed.
    CopyFailed {
        /// Copy source path.
        from: PathBuf,
        /// Copy destination path.
        to: PathBuf,
        /// Error message from the copy implementation.
        message: String,
    },
    /// The copied CSS failed the sanity check.
    CssInvalid {
        /// CSS path that failed validation.
        path: PathBuf,
        /// Reason the CSS was rejected.
        reason: String,
    },
    /// Traversal of a directory failed.
    WalkFailed {
        /// Directory path that could not be traversed.
        path: PathBuf,
        /// Error message from directory traversal.
        message: String,
    },
}

impl Display for AssetSyncError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPath { path } => {
                write!(formatter, "required path is missing: {}", path.display())
            }
            Self::ExpectedFile { path } => {
                write!(
                    formatter,
                    "expected file but found non-file: {}",
                    path.display()
                )
            }
            Self::ExpectedDir { path } => {
                write!(
                    formatter,
                    "expected directory but found non-directory: {}",
                    path.display()
                )
            }
            Self::Io { path, source } => {
                write!(formatter, "io error at {}: {source}", path.display())
            }
            Self::CopyFailed { from, to, message } => write!(
                formatter,
                "copy failed from {} to {}: {message}",
                from.display(),
                to.display()
            ),
            Self::CssInvalid { path, reason } => write!(
                formatter,
                "copied CSS failed validation at {}: {reason}",
                path.display()
            ),
            Self::WalkFailed { path, message } => {
                write!(
                    formatter,
                    "directory walk failed at {}: {message}",
                    path.display()
                )
            }
        }
    }
}

impl Error for AssetSyncError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirStats {
    files: u64,
    bytes: u64,
}

/// Run the asset synchronization using the repository-relative paths.
///
/// # Errors
/// Returns an error if vendor inputs are missing, outputs cannot be written,
/// or the copied CSS fails the sanity check.
pub fn run() -> Result<(), AssetSyncError> {
    let ui_root = ui_root_dir()?;
    sync_assets(&ui_root)
}

fn ui_root_dir() -> Result<PathBuf, AssetSyncError> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ui_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| AssetSyncError::MissingPath {
            path: manifest_dir.to_path_buf(),
        })?;
    Ok(ui_root.to_path_buf())
}

fn sync_assets(ui_root: &Path) -> Result<(), AssetSyncError> {
    let vendor_css = ui_root.join(VENDOR_ROOT).join("html/assets/app.css");
    let vendor_images = ui_root.join(VENDOR_ROOT).join("html/images");
    let vendor_js = ui_root.join(VENDOR_ROOT).join("public/js");

    ensure_file(&vendor_css)?;
    ensure_dir(&vendor_images)?;
    ensure_dir(&vendor_js)?;

    let output_root = ui_root.join(OUTPUT_ROOT);
    let output_assets = output_root.join("assets");
    let output_css = output_assets.join("app.css");
    let output_images = output_root.join("images");
    let output_js = output_root.join("js");

    ensure_dir_exists(&output_assets)?;

    copy_file(&vendor_css, &output_css)?;
    copy_dir(&vendor_images, &output_images)?;
    copy_dir(&vendor_js, &output_js)?;

    validate_css(&output_css)?;

    let css_hash = sha256_hex(&output_css)?;
    let images_stats = dir_stats(&output_images)?;
    let js_stats = dir_stats(&output_js)?;

    write_lock(&output_root, &css_hash, images_stats, js_stats)?;

    Ok(())
}

fn ensure_file(path: &Path) -> Result<(), AssetSyncError> {
    if !path.exists() {
        return Err(AssetSyncError::MissingPath {
            path: path.to_path_buf(),
        });
    }
    if !path.is_file() {
        return Err(AssetSyncError::ExpectedFile {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn ensure_dir(path: &Path) -> Result<(), AssetSyncError> {
    if !path.exists() {
        return Err(AssetSyncError::MissingPath {
            path: path.to_path_buf(),
        });
    }
    if !path.is_dir() {
        return Err(AssetSyncError::ExpectedDir {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn ensure_dir_exists(path: &Path) -> Result<(), AssetSyncError> {
    fs::create_dir_all(path).map_err(|source| AssetSyncError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn copy_file(from: &Path, to: &Path) -> Result<(), AssetSyncError> {
    if let Some(parent) = to.parent() {
        ensure_dir_exists(parent)?;
    }
    fs::copy(from, to).map_err(|source| AssetSyncError::CopyFailed {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        message: source.to_string(),
    })?;
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), AssetSyncError> {
    if to.exists() {
        if to.is_dir() {
            fs::remove_dir_all(to).map_err(|source| AssetSyncError::Io {
                path: to.to_path_buf(),
                source,
            })?;
        } else {
            fs::remove_file(to).map_err(|source| AssetSyncError::Io {
                path: to.to_path_buf(),
                source,
            })?;
        }
    }
    let parent = to.parent().ok_or_else(|| AssetSyncError::MissingPath {
        path: to.to_path_buf(),
    })?;
    ensure_dir_exists(parent)?;
    let mut options = CopyOptions::new();
    options.overwrite = true;
    fs_extra::dir::copy(from, parent, &options).map_err(|err| AssetSyncError::CopyFailed {
        from: from.to_path_buf(),
        to: parent.to_path_buf(),
        message: err.to_string(),
    })?;
    Ok(())
}

fn validate_css(path: &Path) -> Result<(), AssetSyncError> {
    let contents = fs::read_to_string(path).map_err(|source| AssetSyncError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if contents.len() < MIN_CSS_BYTES {
        return Err(AssetSyncError::CssInvalid {
            path: path.to_path_buf(),
            reason: format!("expected at least {MIN_CSS_BYTES} bytes"),
        });
    }
    if !contents.contains(CSS_MARKER) {
        return Err(AssetSyncError::CssInvalid {
            path: path.to_path_buf(),
            reason: format!("missing marker {CSS_MARKER}"),
        });
    }
    Ok(())
}

fn sha256_hex(path: &Path) -> Result<String, AssetSyncError> {
    let bytes = fs::read(path).map_err(|source| AssetSyncError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn dir_stats(path: &Path) -> Result<DirStats, AssetSyncError> {
    let mut files = 0_u64;
    let mut bytes = 0_u64;
    for entry in WalkDir::new(path).min_depth(1) {
        let entry = entry.map_err(|err| AssetSyncError::WalkFailed {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?;
        if entry.file_type().is_file() {
            let metadata = entry.metadata().map_err(|err| AssetSyncError::WalkFailed {
                path: entry.path().to_path_buf(),
                message: err.to_string(),
            })?;
            files += 1;
            bytes += metadata.len();
        }
    }
    Ok(DirStats { files, bytes })
}

fn write_lock(
    output_root: &Path,
    css_hash: &str,
    images: DirStats,
    js: DirStats,
) -> Result<(), AssetSyncError> {
    ensure_dir_exists(output_root)?;
    let lock_path = output_root.join("ASSET_LOCK.txt");
    let contents = format!(
        "app.css sha256 {css_hash}\nimages files {} bytes {}\njs files {} bytes {}\n",
        images.files, images.bytes, js.files, js.bytes
    );
    fs::write(&lock_path, contents).map_err(|source| AssetSyncError::Io {
        path: lock_path,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);
    type TestResult = Result<(), Box<dyn Error>>;

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

    fn css_fixture() -> String {
        let mut css = String::from("/* test */\n.btn { display: inline-flex; }\n");
        let filler = "/* filler */\n";
        while css.len() < MIN_CSS_BYTES {
            css.push_str(filler);
        }
        css
    }

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new() -> Result<Self, std::io::Error> {
            let pid = std::process::id();
            loop {
                let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
                let mut path =
                    server_root().map_err(|err| std::io::Error::other(err.to_string()))?;
                path.push(format!("asset-sync-test-{pid}-{counter}"));
                match fs::create_dir(&path) {
                    Ok(()) => return Ok(Self { path }),
                    Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(err) => return Err(err),
                }
            }
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_vendor_fixture(root: &Path, css: &str) -> Result<(), std::io::Error> {
        let css_path = root.join(VENDOR_ROOT).join("html/assets/app.css");
        if let Some(parent) = css_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&css_path, css)?;

        let images_path = root.join(VENDOR_ROOT).join("html/images");
        fs::create_dir_all(&images_path)?;
        fs::write(images_path.join("logo.png"), "png")?;

        let js_path = root.join(VENDOR_ROOT).join("public/js");
        fs::create_dir_all(&js_path)?;
        fs::write(js_path.join("app.js"), "console.log('ok');")?;
        Ok(())
    }

    #[test]
    fn sync_assets_writes_outputs_and_lock() -> TestResult {
        let temp_root = TempRoot::new()?;
        let css_content = css_fixture();
        write_vendor_fixture(&temp_root.path, &css_content)?;

        sync_assets(&temp_root.path)?;

        let output_css = temp_root.path.join(OUTPUT_ROOT).join("assets/app.css");
        let css_contents = fs::read_to_string(&output_css)?;
        assert_eq!(css_contents, css_content);

        let lock_path = temp_root.path.join(OUTPUT_ROOT).join("ASSET_LOCK.txt");
        let lock_contents = fs::read_to_string(&lock_path)?;
        let css_hash = sha256_hex(&output_css)?;
        assert!(lock_contents.contains(&format!("app.css sha256 {css_hash}")));

        let images_dir = temp_root.path.join(OUTPUT_ROOT).join("images");
        assert!(images_dir.join("logo.png").is_file());
        let js_dir = temp_root.path.join(OUTPUT_ROOT).join("js");
        assert!(js_dir.join("app.js").is_file());
        Ok(())
    }

    #[test]
    fn invalid_css_fails_validation() -> TestResult {
        let temp_root = TempRoot::new()?;
        write_vendor_fixture(&temp_root.path, "body { color: black; }")?;

        let result = sync_assets(&temp_root.path);
        assert!(
            matches!(result, Err(AssetSyncError::CssInvalid { .. })),
            "expected CssInvalid error, got {result:?}"
        );
        Ok(())
    }

    #[test]
    fn asset_sync_error_display_and_source_match_variant() {
        let missing_variant = AssetSyncError::MissingPath {
            path: PathBuf::from("static/nexus/missing.css"),
        };
        assert!(
            missing_variant
                .to_string()
                .contains("required path is missing")
        );
        assert!(missing_variant.source().is_none());

        let expected_file_variant = AssetSyncError::ExpectedFile {
            path: PathBuf::from("static/nexus/images"),
        };
        assert!(
            expected_file_variant
                .to_string()
                .contains("expected file but found non-file")
        );
        assert!(expected_file_variant.source().is_none());

        let expected_dir_variant = AssetSyncError::ExpectedDir {
            path: PathBuf::from("static/nexus/assets/app.css"),
        };
        assert!(
            expected_dir_variant
                .to_string()
                .contains("expected directory but found non-directory")
        );
        assert!(expected_dir_variant.source().is_none());

        let io_error = std::io::Error::other("disk full");
        let io_variant = AssetSyncError::Io {
            path: PathBuf::from("static/nexus/app.css"),
            source: io_error,
        };
        assert!(io_variant.to_string().contains("io error at"));
        assert!(io_variant.source().is_some());

        let copy_variant = AssetSyncError::CopyFailed {
            from: PathBuf::from("ui_vendor/app.css"),
            to: PathBuf::from("static/nexus/assets"),
            message: "permission denied".to_string(),
        };
        assert!(copy_variant.to_string().contains("copy failed from"));
        assert!(copy_variant.source().is_none());

        let css_variant = AssetSyncError::CssInvalid {
            path: PathBuf::from("static/nexus/app.css"),
            reason: "missing marker".to_string(),
        };
        assert!(
            css_variant
                .to_string()
                .contains("copied CSS failed validation")
        );
        assert!(css_variant.source().is_none());

        let walk_variant = AssetSyncError::WalkFailed {
            path: PathBuf::from("static/nexus/images"),
            message: "not readable".to_string(),
        };
        assert!(
            walk_variant
                .to_string()
                .contains("directory walk failed at")
        );
        assert!(walk_variant.source().is_none());
    }

    #[test]
    fn ui_root_dir_resolves_to_revaer_ui_crate() -> TestResult {
        let ui_root = ui_root_dir()?;
        assert!(ui_root.ends_with("crates/revaer-ui"));
        Ok(())
    }

    #[test]
    fn ensure_file_errors_for_missing_and_non_file() {
        let temp_root = TempRoot::new().expect("temp root");
        let missing = temp_root.path.join("missing.css");
        let err = ensure_file(&missing).expect_err("expected missing path error");
        assert!(matches!(err, AssetSyncError::MissingPath { .. }));

        let dir_path = temp_root.path.join("dir");
        fs::create_dir_all(&dir_path).expect("create dir");
        let err = ensure_file(&dir_path).expect_err("expected non-file error");
        assert!(matches!(err, AssetSyncError::ExpectedFile { .. }));
    }

    #[test]
    fn ensure_dir_errors_for_missing_and_non_dir() {
        let temp_root = TempRoot::new().expect("temp root");
        let missing = temp_root.path.join("missing");
        let err = ensure_dir(&missing).expect_err("expected missing path error");
        assert!(matches!(err, AssetSyncError::MissingPath { .. }));

        let file_path = temp_root.path.join("file.txt");
        fs::write(&file_path, "data").expect("write file");
        let err = ensure_dir(&file_path).expect_err("expected non-dir error");
        assert!(matches!(err, AssetSyncError::ExpectedDir { .. }));
    }

    #[test]
    fn copy_file_creates_parent_directories_and_preserves_contents() -> TestResult {
        let temp_root = TempRoot::new()?;
        let source = temp_root.path.join("vendor/app.css");
        if let Some(parent) = source.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&source, css_fixture())?;

        let destination = temp_root.path.join("out/assets/app.css");
        copy_file(&source, &destination)?;

        assert_eq!(fs::read_to_string(destination)?, css_fixture());
        Ok(())
    }

    #[test]
    fn copy_file_reports_missing_source() {
        let temp_root = TempRoot::new().expect("temp root");
        let missing = temp_root.path.join("missing.css");
        let destination = temp_root.path.join("dest/app.css");
        let err = copy_file(&missing, &destination).expect_err("expected copy failure");
        assert!(matches!(err, AssetSyncError::CopyFailed { .. }));
    }

    #[test]
    fn copy_dir_replaces_existing_file_target() -> TestResult {
        let temp_root = TempRoot::new()?;
        let source = temp_root.path.join("source-dir");
        fs::create_dir_all(&source)?;
        fs::write(source.join("asset.txt"), "asset")?;

        let destination_parent = temp_root.path.join("dest");
        fs::create_dir_all(&destination_parent)?;
        let destination = destination_parent.join("source-dir");
        fs::write(&destination, "placeholder")?;

        copy_dir(&source, &destination)?;

        let copied_file = destination.join("asset.txt");
        assert!(copied_file.is_file());
        Ok(())
    }

    #[test]
    fn copy_dir_replaces_existing_directory_target() -> TestResult {
        let temp_root = TempRoot::new()?;
        let source = temp_root.path.join("source-dir");
        fs::create_dir_all(&source)?;
        fs::write(source.join("asset.txt"), "asset")?;

        let destination_parent = temp_root.path.join("dest");
        let destination = destination_parent.join("source-dir");
        fs::create_dir_all(&destination)?;
        fs::write(destination.join("stale.txt"), "stale")?;

        copy_dir(&source, &destination)?;

        assert!(!destination.join("stale.txt").exists());
        assert_eq!(fs::read_to_string(destination.join("asset.txt"))?, "asset");
        Ok(())
    }

    #[test]
    fn copy_dir_reports_missing_source_directory() -> TestResult {
        let temp_root = TempRoot::new()?;
        let source = temp_root.path.join("missing-dir");
        let destination = temp_root.path.join("dest/missing-dir");
        let err = copy_dir(&source, &destination).expect_err("expected copy failure");
        assert!(matches!(err, AssetSyncError::CopyFailed { .. }));
        Ok(())
    }

    #[test]
    fn validate_css_flags_short_or_missing_marker() -> TestResult {
        let temp_root = TempRoot::new()?;
        let short_css = temp_root.path.join("short.css");
        fs::write(&short_css, ".btn{}")?;
        let err = validate_css(&short_css).expect_err("expected short css error");
        assert!(matches!(err, AssetSyncError::CssInvalid { .. }));

        let marker_missing = temp_root.path.join("no-marker.css");
        let mut contents = String::from("body { color: black; }\n");
        while contents.len() < MIN_CSS_BYTES {
            contents.push_str("/* filler */\n");
        }
        fs::write(&marker_missing, contents)?;
        let err = validate_css(&marker_missing).expect_err("expected missing marker error");
        assert!(matches!(err, AssetSyncError::CssInvalid { .. }));
        Ok(())
    }

    #[test]
    fn validate_css_accepts_fixture_content() -> TestResult {
        let temp_root = TempRoot::new()?;
        let css_path = temp_root.path.join("valid.css");
        fs::write(&css_path, css_fixture())?;
        validate_css(&css_path)?;
        Ok(())
    }

    #[test]
    fn validate_css_reports_io_for_missing_file() {
        let missing = PathBuf::from("missing.css");
        let err = validate_css(&missing).expect_err("expected io error");
        assert!(matches!(err, AssetSyncError::Io { .. }));
    }

    #[test]
    fn sha256_hex_reports_missing_path() {
        let missing = PathBuf::from("no-such-file");
        let err = sha256_hex(&missing).expect_err("expected hash error");
        assert!(matches!(err, AssetSyncError::Io { .. }));
    }

    #[test]
    fn dir_stats_counts_files_and_reports_errors() -> TestResult {
        let temp_root = TempRoot::new()?;
        let dir = temp_root.path.join("stats");
        fs::create_dir_all(dir.join("nested"))?;
        fs::write(dir.join("a.txt"), "a")?;
        fs::write(dir.join("nested/b.txt"), "b")?;

        let stats = dir_stats(&dir)?;
        assert_eq!(stats.files, 2);
        assert!(stats.bytes >= 2);

        let missing = temp_root.path.join("missing");
        let err = dir_stats(&missing).expect_err("expected walk error");
        assert!(matches!(err, AssetSyncError::WalkFailed { .. }));
        Ok(())
    }

    #[test]
    fn write_lock_reports_io_error_when_output_is_file() -> TestResult {
        let temp_root = TempRoot::new()?;
        let output_root = temp_root.path.join("output");
        fs::write(&output_root, "not a dir")?;

        let err = write_lock(
            &output_root,
            "hash",
            DirStats { files: 0, bytes: 0 },
            DirStats { files: 0, bytes: 0 },
        )
        .expect_err("expected io error");
        assert!(matches!(err, AssetSyncError::Io { .. }));
        Ok(())
    }

    #[test]
    fn write_lock_records_hash_and_directory_stats() -> TestResult {
        let temp_root = TempRoot::new()?;
        let output_root = temp_root.path.join("output");
        write_lock(
            &output_root,
            "abc123",
            DirStats {
                files: 2,
                bytes: 20,
            },
            DirStats {
                files: 1,
                bytes: 10,
            },
        )?;
        let contents = fs::read_to_string(output_root.join("ASSET_LOCK.txt"))?;
        assert!(contents.contains("app.css sha256 abc123"));
        assert!(contents.contains("images files 2 bytes 20"));
        assert!(contents.contains("js files 1 bytes 10"));
        Ok(())
    }

    #[test]
    fn write_lock_reports_io_error_when_lock_path_is_directory() -> TestResult {
        let temp_root = TempRoot::new()?;
        let output_root = temp_root.path.join("output");
        fs::create_dir_all(output_root.join("ASSET_LOCK.txt"))?;

        let err = write_lock(
            &output_root,
            "hash",
            DirStats { files: 1, bytes: 1 },
            DirStats { files: 1, bytes: 1 },
        )
        .expect_err("expected lock write error");
        assert!(matches!(err, AssetSyncError::Io { .. }));
        Ok(())
    }
}
