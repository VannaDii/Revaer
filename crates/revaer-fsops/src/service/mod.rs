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

//! Filesystem post-processing pipeline for completed torrents.

use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, MutexGuard},
};

#[cfg(test)]
use std::collections::HashMap;

use crate::error::{FsOpsError, FsOpsResult};
use crate::model::FsOpsRequest;
use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use globset::{Glob, GlobSet, GlobSetBuilder};
use revaer_config::FsPolicy;
use revaer_events::{Event, EventBus};
use revaer_runtime::RuntimeStore;
use revaer_telemetry::Metrics;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tar::Archive as TarArchive;
use tracing::{error, info, warn};
use uuid::Uuid;
use walkdir::WalkDir;
use zip::ZipArchive;

#[cfg(all(unix, test))]
use std::os::unix::fs::MetadataExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(unix)]
use nix::unistd::{Gid, Group, Uid, User, chown};

const META_DIR_NAME: &str = ".revaer";
const META_SUFFIX: &str = ".meta.json";
const HEALTH_COMPONENT: &str = "fsops";
const SKIP_FLUFF_PRESET: &str = "@skip_fluff";
const SKIP_FLUFF_PATTERNS: &[&str] = &[
    "**/sample/**",
    "**/samples/**",
    "**/extras/**",
    "**/proof/**",
    "**/screens/**",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StepKind {
    ValidatePolicy,
    Allowlist,
    PrepareDirectories,
    CompileRules,
    LocateSource,
    VerifyPar2,
    PrepareWorkDir,
    Extract,
    Flatten,
    Transfer,
    SetPermissions,
    Cleanup,
    Checksum,
    Finalise,
}

impl StepKind {
    const fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::ValidatePolicy => ("validate_policy", "validate"),
            Self::Allowlist => ("allowlist", "allowlist"),
            Self::PrepareDirectories => ("prepare_directories", "prepare_directories"),
            Self::CompileRules => ("compile_rules", "compile_rules"),
            Self::LocateSource => ("locate_source", "locate_source"),
            Self::VerifyPar2 => ("verify_par2", "verify_par2"),
            Self::PrepareWorkDir => ("prepare_work_dir", "prepare_work_dir"),
            Self::Extract => ("extract", "extract"),
            Self::Flatten => ("flatten", "flatten"),
            Self::Transfer => ("transfer", "transfer"),
            Self::SetPermissions => ("set_permissions", "set_permissions"),
            Self::Cleanup => ("cleanup", "cleanup"),
            Self::Checksum => ("checksum", "checksum"),
            Self::Finalise => ("finalise", "finalise"),
        }
    }

    const fn as_str(self) -> &'static str {
        self.names().0
    }

    const fn progress_label(self) -> &'static str {
        self.names().1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StepStatus {
    Started,
    Completed,
    Failed,
    Skipped,
}

impl StepStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StepRecord {
    name: String,
    status: StepStatus,
    detail: Option<String>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone, Copy)]
struct StepPersistence {
    start: bool,
    success: bool,
    failure: bool,
}

impl StepPersistence {
    const fn new(start: bool, success: bool, failure: bool) -> Self {
        Self {
            start,
            success,
            failure,
        }
    }
}

enum StepOutcome {
    Completed(Option<String>),
    Skipped(Option<String>),
}

impl StepOutcome {
    const fn status(&self) -> StepStatus {
        match self {
            Self::Completed(_) => StepStatus::Completed,
            Self::Skipped(_) => StepStatus::Skipped,
        }
    }

    fn detail(&self) -> Option<&str> {
        match self {
            Self::Completed(detail) | Self::Skipped(detail) => detail.as_deref(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Par2Mode {
    Disabled,
    Verify,
    Repair,
}

impl Par2Mode {
    fn from_policy(value: &str) -> FsOpsResult<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "disabled" | "off" => Ok(Self::Disabled),
            "verify" | "enabled" => Ok(Self::Verify),
            "repair" => Ok(Self::Repair),
            other => Err(FsOpsError::InvalidPolicy {
                field: "par2",
                reason: "unsupported",
                value: Some(other.to_string()),
            }),
        }
    }

    const fn command(self) -> &'static str {
        match self {
            Self::Disabled | Self::Verify => "v",
            Self::Repair => "r",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Verify => "verify",
            Self::Repair => "repair",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArchiveFormat {
    Zip,
    Tar,
    TarGz,
    SevenZip,
    Rar,
}

impl ArchiveFormat {
    fn detect(source: &Path) -> FsOpsResult<Self> {
        let file_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| FsOpsError::InvalidInput {
                field: "archive_extension",
                reason: "missing",
                value: Some(source.to_string_lossy().into_owned()),
            })?;

        if !file_name.contains('.') {
            return Err(FsOpsError::InvalidInput {
                field: "archive_extension",
                reason: "missing",
                value: Some(source.to_string_lossy().into_owned()),
            });
        }

        let path_name = Path::new(&file_name);
        if file_name.ends_with(".tar.gz")
            || path_name
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("tgz"))
        {
            return Ok(Self::TarGz);
        }
        if path_name
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("tar"))
        {
            return Ok(Self::Tar);
        }
        if path_name
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            return Ok(Self::Zip);
        }
        if path_name
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("7z"))
        {
            return Ok(Self::SevenZip);
        }
        if path_name
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("rar"))
        {
            return Ok(Self::Rar);
        }

        Err(FsOpsError::Unsupported {
            operation: "extract_archive",
            value: Some(file_name),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ChecksumKind {
    File,
    Manifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ChecksumRecord {
    path: String,
    kind: ChecksumKind,
    algorithm: String,
    value: String,
    bytes: u64,
}

impl ChecksumRecord {
    fn with_path(mut self, path: String) -> Self {
        self.path = path;
        self
    }
}

#[derive(Debug, Clone)]
struct ExternalCommand {
    program: &'static str,
    args: Vec<String>,
}

#[derive(Clone, Copy)]
struct ArchiveWriteOps {
    create_dir: &'static str,
    create_parent: &'static str,
    create_file: &'static str,
    copy: &'static str,
    set_permissions: &'static str,
}

impl ArchiveWriteOps {
    const fn new(
        create_dir: &'static str,
        create_parent: &'static str,
        create_file: &'static str,
        copy: &'static str,
        set_permissions: &'static str,
    ) -> Self {
        Self {
            create_dir,
            create_parent,
            create_file,
            copy,
            set_permissions,
        }
    }
}

const ZIP_EXTRACT_OPS: ArchiveWriteOps = ArchiveWriteOps::new(
    "extract_zip.create_dir",
    "extract_zip.create_parent",
    "extract_zip.create_file",
    "extract_zip.copy",
    "extract_zip.set_permissions",
);

const TAR_EXTRACT_OPS: ArchiveWriteOps = ArchiveWriteOps::new(
    "extract_tar.create_dir",
    "extract_tar.create_parent",
    "extract_tar.create_file",
    "extract_tar.copy",
    "extract_tar.set_permissions",
);

#[derive(Clone, Copy)]
struct TreeTransferOps {
    walk: &'static str,
    create_dir: &'static str,
    create_parent: &'static str,
    transfer_file: &'static str,
    transfer_entry: &'static str,
}

impl TreeTransferOps {
    const fn new(
        walk: &'static str,
        create_dir: &'static str,
        create_parent: &'static str,
        transfer_file: &'static str,
        transfer_entry: &'static str,
    ) -> Self {
        Self {
            walk,
            create_dir,
            create_parent,
            transfer_file,
            transfer_entry,
        }
    }
}

const COPY_TREE_OPS: TreeTransferOps = TreeTransferOps::new(
    "copy_tree.walk",
    "copy_tree.create_dir",
    "copy_tree.create_parent",
    "copy_tree.copy_file",
    "copy_tree.copy_entry",
);

const HARDLINK_TREE_OPS: TreeTransferOps = TreeTransferOps::new(
    "hardlink_tree.walk",
    "hardlink_tree.create_dir",
    "hardlink_tree.create_parent",
    "hardlink_tree.link_file",
    "hardlink_tree.link_entry",
);

/// Service responsible for executing filesystem post-processing steps after torrent completion.
#[derive(Clone)]
pub struct FsOpsService {
    events: EventBus,
    metrics: Metrics,
    health_degraded: Arc<Mutex<bool>>,
    runtime: Option<RuntimeStore>,
}

impl FsOpsService {
    /// Construct a new filesystem operations service backed by the shared event bus.
    #[must_use]
    pub fn new(events: EventBus, metrics: Metrics) -> Self {
        Self {
            events,
            metrics,
            health_degraded: Arc::new(Mutex::new(false)),
            runtime: None,
        }
    }

    /// Attach a runtime store used for persistence.
    #[must_use]
    pub fn with_runtime(mut self, runtime: RuntimeStore) -> Self {
        self.runtime = Some(runtime);
        self
    }

    /// Apply the configured filesystem policy for the given torrent and emit progress events.
    ///
    /// # Errors
    ///
    /// Returns an error if any filesystem post-processing step fails.
    pub fn apply(&self, request: FsOpsRequest<'_>) -> FsOpsResult<()> {
        self.publish_event(Event::FsopsStarted {
            torrent_id: request.torrent_id,
        });

        self.record_job_started(request.torrent_id, request.source_path);

        match self.execute_pipeline(&request) {
            Ok(meta) => {
                self.mark_recovered();
                self.record_job_completed(request.torrent_id, &meta);
                self.publish_event(Event::FsopsCompleted {
                    torrent_id: request.torrent_id,
                });
                Ok(())
            }
            Err(error) => {
                let detail = format!("{error:#}");
                self.mark_degraded(&detail);
                self.record_job_failed(request.torrent_id, request.source_path, detail.clone());
                self.publish_event(Event::FsopsFailed {
                    torrent_id: request.torrent_id,
                    message: detail,
                });
                Err(error)
            }
        }
    }

    fn execute_pipeline(&self, request: &FsOpsRequest<'_>) -> FsOpsResult<FsOpsMeta> {
        let torrent_id = request.torrent_id;
        let policy = request.policy;
        let source_path = request.source_path;

        let root = PathBuf::from(&policy.library_root);
        let meta_dir = root.join(META_DIR_NAME);
        let meta_path = meta_dir.join(format!("{torrent_id}{META_SUFFIX}"));

        let mut meta =
            self.load_or_initialise_meta(torrent_id, policy.id, &meta_path, source_path)?;

        if meta.completed {
            self.emit_progress(torrent_id, "resume");
            info!(torrent_id = %torrent_id, "fsops already completed; skipping");
            return Ok(meta);
        }

        self.run_validate_policy(torrent_id, &mut meta, &meta_path, policy)?;
        self.run_allowlist(torrent_id, &mut meta, &meta_path, policy, &root)?;
        self.run_prepare_directories(torrent_id, &mut meta, &meta_path, &root, &meta_dir)?;
        self.run_compile_rules(torrent_id, &mut meta, &meta_path, policy)?;
        self.run_locate_source(torrent_id, &mut meta, &meta_path, source_path)?;
        self.run_verify_par2(torrent_id, &mut meta, &meta_path, policy)?;
        self.run_prepare_work_dir(torrent_id, &mut meta, &meta_path, &meta_dir)?;
        self.run_extract(torrent_id, &mut meta, &meta_path, policy)?;
        self.run_flatten(torrent_id, &mut meta, &meta_path, policy)?;
        self.run_transfer(torrent_id, &mut meta, &meta_path, policy, &root)?;
        self.run_set_permissions(torrent_id, &mut meta, &meta_path, policy)?;
        self.run_cleanup(torrent_id, &mut meta, &meta_path, policy)?;
        self.run_checksum(torrent_id, &mut meta, &meta_path)?;
        self.run_finalise(torrent_id, &mut meta, &meta_path)?;

        Ok(meta)
    }

    fn run_validate_policy(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
        policy: &FsPolicy,
    ) -> FsOpsResult<()> {
        let root_value = policy.library_root.as_str();
        self.execute_step(
            torrent_id,
            meta,
            meta_path,
            StepKind::ValidatePolicy,
            StepPersistence::new(false, false, false),
            |_meta| {
                if root_value.trim().is_empty() {
                    return Err(FsOpsError::InvalidPolicy {
                        field: "library_root",
                        reason: "empty",
                        value: Some(root_value.to_string()),
                    });
                }
                Ok(StepOutcome::Completed(Some(format!(
                    "library_root={root_value}"
                ))))
            },
        )
    }

    fn run_allowlist(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
        policy: &FsPolicy,
        root: &Path,
    ) -> FsOpsResult<()> {
        let allow_paths = policy.allow_paths.clone();
        let root_clone = root.to_path_buf();
        self.execute_step(
            torrent_id,
            meta,
            meta_path,
            StepKind::Allowlist,
            StepPersistence::new(false, false, false),
            move |_meta| {
                enforce_allow_paths(&root_clone, &allow_paths)?;
                Ok(StepOutcome::Completed(None))
            },
        )
    }

    fn run_prepare_directories(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
        root: &Path,
        meta_dir: &Path,
    ) -> FsOpsResult<()> {
        let root_clone = root.to_path_buf();
        let meta_dir_clone = meta_dir.to_path_buf();
        self.execute_step(
            torrent_id,
            meta,
            meta_path,
            StepKind::PrepareDirectories,
            StepPersistence::new(false, true, false),
            move |_meta| {
                fs::create_dir_all(&root_clone).map_err(|source| {
                    FsOpsError::io("prepare_directories.create_root", &root_clone, source)
                })?;
                fs::create_dir_all(&meta_dir_clone).map_err(|source| {
                    FsOpsError::io(
                        "prepare_directories.create_meta_dir",
                        &meta_dir_clone,
                        source,
                    )
                })?;
                Ok(StepOutcome::Completed(Some(format!(
                    "meta_dir={}",
                    meta_dir_clone.display()
                ))))
            },
        )
    }

    fn run_compile_rules(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
        policy: &FsPolicy,
    ) -> FsOpsResult<()> {
        self.execute_step(
            torrent_id,
            meta,
            meta_path,
            StepKind::CompileRules,
            StepPersistence::new(false, false, false),
            |meta| {
                let rules = RuleSet::from_policy(policy)?;
                meta.updated_at = Utc::now();
                Ok(StepOutcome::Completed(Some(format!(
                    "include_count={} exclude_count={}",
                    rules.include_count(),
                    rules.exclude_count()
                ))))
            },
        )
    }

    fn run_locate_source(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
        source_path: &Path,
    ) -> FsOpsResult<()> {
        let explicit_source = source_path.to_path_buf();
        self.execute_step(
            torrent_id,
            meta,
            meta_path,
            StepKind::LocateSource,
            StepPersistence::new(false, true, false),
            move |meta| {
                let candidate = meta
                    .source_path
                    .as_ref()
                    .map_or_else(|| explicit_source.clone(), PathBuf::from);
                let canonical = candidate
                    .canonicalize()
                    .unwrap_or_else(|_| candidate.clone());
                if !canonical.exists() {
                    return Err(FsOpsError::InvalidInput {
                        field: "source_path",
                        reason: "missing",
                        value: Some(canonical.to_string_lossy().into_owned()),
                    });
                }
                let encoded = canonical.to_string_lossy().into_owned();
                meta.source_path = Some(encoded);
                if meta.staging_path.is_none() {
                    meta.staging_path = meta.source_path.clone();
                }
                Ok(StepOutcome::Completed(Some(format!(
                    "source={}",
                    canonical.display()
                ))))
            },
        )
    }

    fn run_verify_par2(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
        policy: &FsPolicy,
    ) -> FsOpsResult<()> {
        let par2_mode = Par2Mode::from_policy(&policy.par2)?;
        self.execute_step(
            torrent_id,
            meta,
            meta_path,
            StepKind::VerifyPar2,
            StepPersistence::new(false, true, false),
            move |meta| {
                if matches!(par2_mode, Par2Mode::Disabled) {
                    return Ok(StepOutcome::Skipped(Some("par2 disabled".into())));
                }

                let source = meta.source_path.as_ref().map(PathBuf::from).ok_or(
                    FsOpsError::MissingState {
                        field: "source_path",
                    },
                )?;
                let search_root = if source.is_dir() {
                    source.clone()
                } else {
                    source
                        .parent()
                        .map(Path::to_path_buf)
                        .ok_or(FsOpsError::MissingState {
                            field: "source_parent",
                        })?
                };

                let Some(par2_file) = Self::find_par2_file(&search_root, &source)? else {
                    return Ok(StepOutcome::Skipped(Some(
                        "no par2 files discovered".into(),
                    )));
                };

                let detail = Self::run_par2_tool(par2_mode, &search_root, &par2_file)?;
                Ok(StepOutcome::Completed(Some(detail)))
            },
        )
    }

    fn run_prepare_work_dir(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
        meta_dir: &Path,
    ) -> FsOpsResult<()> {
        let seed = meta_dir.join("work");
        let label = torrent_id.to_string();
        self.execute_step(
            torrent_id,
            meta,
            meta_path,
            StepKind::PrepareWorkDir,
            StepPersistence::new(false, true, false),
            move |meta| {
                let default_work_dir = seed.join(&label);
                let work_dir_path = meta
                    .work_dir
                    .as_ref()
                    .map_or_else(|| default_work_dir.clone(), PathBuf::from);
                fs::create_dir_all(&work_dir_path).map_err(|source_err| {
                    FsOpsError::io("prepare_work_dir.create_dir", &work_dir_path, source_err)
                })?;
                meta.work_dir = Some(work_dir_path.to_string_lossy().into_owned());
                Ok(StepOutcome::Completed(Some(format!(
                    "work_dir={}",
                    work_dir_path.display()
                ))))
            },
        )
    }

    fn run_extract(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
        policy: &FsPolicy,
    ) -> FsOpsResult<()> {
        let extract_enabled = policy.extract;
        self.execute_step(
            torrent_id,
            meta,
            meta_path,
            StepKind::Extract,
            StepPersistence::new(false, true, false),
            move |meta| {
                if !extract_enabled {
                    if meta.staging_path.is_none() {
                        meta.staging_path = meta.source_path.clone();
                    }
                    return Ok(StepOutcome::Skipped(Some("extract disabled".into())));
                }
                let staging = if let Some(path) = meta.staging_path.as_ref() {
                    PathBuf::from(path)
                } else if let Some(source) = meta.source_path.as_ref() {
                    PathBuf::from(source)
                } else {
                    return Err(FsOpsError::MissingState {
                        field: "source_path",
                    });
                };
                if staging.is_dir() {
                    return Ok(StepOutcome::Skipped(Some(
                        "source already directory; nothing to extract".into(),
                    )));
                }
                let work_dir = meta
                    .work_dir
                    .as_ref()
                    .map(PathBuf::from)
                    .ok_or(FsOpsError::MissingState { field: "work_dir" })?;
                let extraction_target = work_dir.join("extracted");
                if extraction_target.exists() {
                    fs::remove_dir_all(&extraction_target).map_err(|source| {
                        FsOpsError::io("extract.reset_directory", &extraction_target, source)
                    })?;
                }
                fs::create_dir_all(&extraction_target).map_err(|source| {
                    FsOpsError::io("extract.create_directory", &extraction_target, source)
                })?;
                Self::extract_archive(&staging, &extraction_target)?;
                meta.staging_path = Some(extraction_target.to_string_lossy().into_owned());
                Ok(StepOutcome::Completed(Some(format!(
                    "archive={}",
                    staging.display()
                ))))
            },
        )
    }

    fn run_flatten(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
        policy: &FsPolicy,
    ) -> FsOpsResult<()> {
        let flatten_enabled = policy.flatten;
        self.execute_step(
            torrent_id,
            meta,
            meta_path,
            StepKind::Flatten,
            StepPersistence::new(false, true, false),
            move |meta| {
                if !flatten_enabled {
                    return Ok(StepOutcome::Skipped(Some("flatten disabled".into())));
                }
                let staging = meta.staging_path.as_ref().map(PathBuf::from).ok_or(
                    FsOpsError::MissingState {
                        field: "staging_path",
                    },
                )?;
                if !staging.is_dir() {
                    return Ok(StepOutcome::Skipped(Some(
                        "staging path is not a directory".into(),
                    )));
                }
                let mut entries = Vec::new();
                for entry in fs::read_dir(&staging)
                    .map_err(|source| FsOpsError::io("flatten.read_dir", &staging, source))?
                {
                    let entry = entry.map_err(|source| {
                        FsOpsError::io("flatten.read_dir_entry", &staging, source)
                    })?;
                    entries.push(entry);
                }
                if entries.len() != 1 || !entries[0].path().is_dir() {
                    return Ok(StepOutcome::Skipped(Some(
                        "staging directory not a single-nested tree".into(),
                    )));
                }
                let inner = entries.remove(0).path();
                meta.staging_path = Some(inner.to_string_lossy().into_owned());
                Ok(StepOutcome::Completed(Some(format!(
                    "flattened_to={}",
                    inner.display()
                ))))
            },
        )
    }

    fn run_transfer(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
        policy: &FsPolicy,
        root: &Path,
    ) -> FsOpsResult<()> {
        let move_mode = policy.move_mode.as_str();
        let root_clone = root.to_path_buf();
        let torrent_label = torrent_id.to_string();
        self.execute_step(
            torrent_id,
            meta,
            meta_path,
            StepKind::Transfer,
            StepPersistence::new(false, true, false),
            move |meta| {
                let staging = meta.staging_path.as_ref().map(PathBuf::from).ok_or(
                    FsOpsError::MissingState {
                        field: "staging_path",
                    },
                )?;
                let destination = meta.artifact_path.as_ref().map_or_else(
                    || {
                        let inferred = staging
                            .file_name()
                            .and_then(|name| name.to_str())
                            .filter(|name| !name.is_empty())
                            .map_or_else(|| torrent_label.clone(), std::borrow::ToOwned::to_owned);
                        root_clone.join(inferred)
                    },
                    PathBuf::from,
                );

                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|source| {
                        FsOpsError::io("transfer.create_parent", parent, source)
                    })?;
                }

                if destination.exists() {
                    if staging.canonicalize().ok() == destination.canonicalize().ok() {
                        meta.transfer_mode = Some(move_mode.to_string());
                        meta.staging_path = Some(destination.to_string_lossy().into_owned());
                        meta.artifact_path = Some(destination.to_string_lossy().into_owned());
                        return Ok(StepOutcome::Skipped(Some(
                            "artifact already positioned".into(),
                        )));
                    }
                    if destination.is_file() {
                        fs::remove_file(&destination).map_err(|source| {
                            FsOpsError::io("transfer.remove_file", &destination, source)
                        })?;
                    } else {
                        fs::remove_dir_all(&destination).map_err(|source| {
                            FsOpsError::io("transfer.remove_dir", &destination, source)
                        })?;
                    }
                }

                match move_mode {
                    "copy" => Self::copy_tree(&staging, &destination)?,
                    "move" => Self::move_tree(&staging, &destination)?,
                    "hardlink" => Self::hardlink_tree(&staging, &destination)?,
                    other => {
                        return Err(FsOpsError::InvalidPolicy {
                            field: "move_mode",
                            reason: "unsupported",
                            value: Some(other.to_string()),
                        });
                    }
                }

                let mode_string = move_mode.to_string();
                meta.transfer_mode = Some(mode_string);
                meta.artifact_path = Some(destination.to_string_lossy().into_owned());
                meta.staging_path = meta.artifact_path.clone();
                Ok(StepOutcome::Completed(Some(format!(
                    "destination={}",
                    destination.display()
                ))))
            },
        )
    }

    fn run_set_permissions(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
        policy: &FsPolicy,
    ) -> FsOpsResult<()> {
        let chmod_file = policy.chmod_file.clone();
        let chmod_dir = policy.chmod_dir.clone();
        let owner = policy.owner.clone();
        let group = policy.group.clone();
        let umask = policy.umask.clone();
        self.execute_step(
            torrent_id,
            meta,
            meta_path,
            StepKind::SetPermissions,
            StepPersistence::new(false, true, false),
            move |meta| {
                let artifact = match meta.artifact_path.as_ref() {
                    Some(path) => PathBuf::from(path),
                    None => {
                        return Ok(StepOutcome::Skipped(Some(
                            "artifact path unavailable; skipping permission step".into(),
                        )));
                    }
                };

                if !artifact.exists() {
                    return Ok(StepOutcome::Skipped(Some(
                        "artifact path missing on disk".into(),
                    )));
                }

                if chmod_file.is_none()
                    && chmod_dir.is_none()
                    && owner.is_none()
                    && group.is_none()
                    && umask.is_none()
                {
                    return Ok(StepOutcome::Skipped(Some(
                        "no permission directives configured".into(),
                    )));
                }

                let detail = Self::apply_permissions(
                    &artifact,
                    chmod_file.as_deref(),
                    chmod_dir.as_deref(),
                    owner.as_deref(),
                    group.as_deref(),
                    umask.as_deref(),
                )?;

                Ok(StepOutcome::Completed(Some(detail)))
            },
        )
    }

    fn run_cleanup(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
        policy: &FsPolicy,
    ) -> FsOpsResult<()> {
        self.execute_step(
            torrent_id,
            meta,
            meta_path,
            StepKind::Cleanup,
            StepPersistence::new(false, true, false),
            |meta| {
                let artifact = match meta.artifact_path.as_ref() {
                    Some(path) => PathBuf::from(path),
                    None => {
                        return Ok(StepOutcome::Skipped(Some(
                            "artifact path unavailable; skipping cleanup".into(),
                        )));
                    }
                };

                if !artifact.exists() || !artifact.is_dir() {
                    return Ok(StepOutcome::Skipped(Some(
                        "artifact is not a directory; cleanup skipped".into(),
                    )));
                }

                let rules = RuleSet::from_policy(policy)?;
                if rules.include_count() == 0 && rules.exclude_count() == 0 {
                    return Ok(StepOutcome::Skipped(Some(
                        "no cleanup rules configured".into(),
                    )));
                }

                let removed = Self::cleanup_destination(&artifact, &rules);
                Ok(StepOutcome::Completed(Some(format!(
                    "removed_entries={removed}"
                ))))
            },
        )
    }

    fn run_checksum(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
    ) -> FsOpsResult<()> {
        self.execute_step(
            torrent_id,
            meta,
            meta_path,
            StepKind::Checksum,
            StepPersistence::new(false, true, false),
            |meta| {
                let artifact = if let Some(path) = meta.artifact_path.as_ref() {
                    PathBuf::from(path)
                } else {
                    meta.checksums.clear();
                    return Ok(StepOutcome::Skipped(Some(
                        "artifact path unavailable; skipping checksum".into(),
                    )));
                };

                if !artifact.exists() {
                    meta.checksums.clear();
                    return Ok(StepOutcome::Skipped(Some(
                        "artifact path missing on disk".into(),
                    )));
                }

                meta.checksums = Self::build_checksums(&artifact)?;
                Ok(StepOutcome::Completed(Some(format!(
                    "checksums={}",
                    meta.checksums.len()
                ))))
            },
        )
    }

    fn run_finalise(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
    ) -> FsOpsResult<()> {
        self.execute_step(
            torrent_id,
            meta,
            meta_path,
            StepKind::Finalise,
            StepPersistence::new(true, true, false),
            |meta| {
                if let Some(work_dir) = meta.work_dir.as_ref().map(PathBuf::from)
                    && work_dir.exists()
                    && let Err(err) = fs::remove_dir_all(&work_dir)
                {
                    warn!(
                        error = %err,
                        path = %work_dir.display(),
                        "failed to remove fsops work directory"
                    );
                }
                meta.completed = true;
                meta.updated_at = Utc::now();
                let detail = meta.artifact_path.as_ref().map_or_else(
                    || "artifact=<unset>".to_string(),
                    |path| format!("artifact={path}"),
                );
                Ok(StepOutcome::Completed(Some(detail)))
            },
        )
    }

    fn emit_progress(&self, torrent_id: Uuid, step: &str) {
        self.publish_event(Event::FsopsProgress {
            torrent_id,
            step: step.to_string(),
        });
    }

    fn load_or_initialise_meta(
        &self,
        torrent_id: Uuid,
        policy_id: Uuid,
        meta_path: &Path,
        source_path: &Path,
    ) -> FsOpsResult<FsOpsMeta> {
        let mut meta = if meta_path.exists() {
            self.emit_progress(torrent_id, "load_meta");
            load_meta(meta_path)?
        } else {
            FsOpsMeta::new(torrent_id, policy_id)
        };

        if meta.source_path.is_none() {
            let canonical = source_path
                .canonicalize()
                .unwrap_or_else(|_| source_path.to_path_buf());
            meta.source_path = Some(canonical.to_string_lossy().into_owned());
        }

        if meta.staging_path.is_none() {
            meta.staging_path = meta.source_path.clone();
        }

        Ok(meta)
    }

    fn execute_step<F>(
        &self,
        torrent_id: Uuid,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
        step: StepKind,
        persistence: StepPersistence,
        op: F,
    ) -> FsOpsResult<()>
    where
        F: FnOnce(&mut FsOpsMeta) -> FsOpsResult<StepOutcome>,
    {
        if meta.step_status(step) == Some(StepStatus::Completed)
            && (step != StepKind::Finalise || meta.completed)
        {
            return Ok(());
        }

        self.emit_progress(torrent_id, step.progress_label());
        self.record_step(
            meta,
            meta_path,
            step,
            StepStatus::Started,
            None,
            persistence.start,
        )?;

        match op(meta) {
            Ok(outcome) => {
                self.record_step(
                    meta,
                    meta_path,
                    step,
                    outcome.status(),
                    outcome.detail(),
                    persistence.success,
                )?;
                Ok(())
            }
            Err(err) => {
                let detail = err.to_string();
                if let Err(record_err) = self.record_step(
                    meta,
                    meta_path,
                    step,
                    StepStatus::Failed,
                    Some(&detail),
                    persistence.failure,
                ) {
                    error!(
                        error = %record_err,
                        step = step.as_str(),
                        "failed to persist fsops failure step"
                    );
                }
                Err(err)
            }
        }
    }

    fn record_step(
        &self,
        meta: &mut FsOpsMeta,
        meta_path: &Path,
        step: StepKind,
        status: StepStatus,
        detail: Option<&str>,
        persist: bool,
    ) -> FsOpsResult<()> {
        let changed = meta.update_step(step, status, detail.map(str::to_string));
        if changed {
            if persist {
                persist_meta(meta_path, meta)?;
            }
            self.metrics.inc_fsops_step(step.as_str(), status.as_str());
        }
        Ok(())
    }

    fn extract_archive(source: &Path, target: &Path) -> FsOpsResult<()> {
        match ArchiveFormat::detect(source)? {
            ArchiveFormat::Zip => Self::extract_zip(source, target),
            ArchiveFormat::Tar => Self::extract_tar(source, target),
            ArchiveFormat::TarGz => Self::extract_tar_gz(source, target),
            ArchiveFormat::SevenZip => {
                let mut candidates = Self::seven_zip_extract_candidates(source, target);
                candidates.push(Self::unar_extract_command(source, target));
                Self::extract_with_external_tool("extract_archive", &candidates)
            }
            ArchiveFormat::Rar => {
                let mut candidates = Self::seven_zip_extract_candidates(source, target);
                candidates.push(Self::unrar_extract_command(source, target));
                candidates.push(Self::unar_extract_command(source, target));
                Self::extract_with_external_tool("extract_archive", &candidates)
            }
        }
    }

    fn seven_zip_extract_candidates(source: &Path, target: &Path) -> Vec<ExternalCommand> {
        let args = vec![
            "x".to_string(),
            "-y".to_string(),
            format!("-o{}", target.display()),
            source.to_string_lossy().into_owned(),
        ];
        vec![
            ExternalCommand {
                program: "7zz",
                args: args.clone(),
            },
            ExternalCommand {
                program: "7z",
                args,
            },
        ]
    }

    fn unar_extract_command(source: &Path, target: &Path) -> ExternalCommand {
        ExternalCommand {
            program: "unar",
            args: vec![
                "-o".to_string(),
                target.to_string_lossy().into_owned(),
                source.to_string_lossy().into_owned(),
            ],
        }
    }

    fn unrar_extract_command(source: &Path, target: &Path) -> ExternalCommand {
        ExternalCommand {
            program: "unrar",
            args: vec![
                "x".to_string(),
                "-o+".to_string(),
                source.to_string_lossy().into_owned(),
                target.to_string_lossy().into_owned(),
            ],
        }
    }

    fn extract_zip(source: &Path, target: &Path) -> FsOpsResult<()> {
        let file = File::open(source)
            .map_err(|source_err| FsOpsError::io("extract_zip.open", source, source_err))?;
        let mut archive = ZipArchive::new(file)
            .map_err(|source_err| FsOpsError::zip("extract_zip.decode", source, source_err))?;

        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).map_err(|source_err| {
                FsOpsError::zip("extract_zip.read_entry", source, source_err)
            })?;
            let entry_path = Self::sanitize_archive_path(entry.name())?;
            let mut destination = target.to_path_buf();
            destination.push(&entry_path);

            if entry.name().ends_with('/') {
                Self::create_directory(&destination, ZIP_EXTRACT_OPS.create_dir)?;
                continue;
            }

            let unix_mode = entry.unix_mode();
            Self::write_archive_file(&mut entry, &destination, ZIP_EXTRACT_OPS, unix_mode)?;
        }

        Ok(())
    }

    fn extract_tar(source: &Path, target: &Path) -> FsOpsResult<()> {
        let file = File::open(source)
            .map_err(|source_err| FsOpsError::io("extract_tar.open", source, source_err))?;
        let archive = TarArchive::new(file);
        Self::extract_tar_entries(source, target, archive)
    }

    fn extract_tar_gz(source: &Path, target: &Path) -> FsOpsResult<()> {
        let file = File::open(source)
            .map_err(|source_err| FsOpsError::io("extract_tar_gz.open", source, source_err))?;
        let archive = TarArchive::new(GzDecoder::new(file));
        Self::extract_tar_entries(source, target, archive)
    }

    fn extract_tar_entries<R: Read>(
        source: &Path,
        target: &Path,
        mut archive: TarArchive<R>,
    ) -> FsOpsResult<()> {
        let entries = archive
            .entries()
            .map_err(|source_err| FsOpsError::io("extract_tar.entries", source, source_err))?;

        for entry in entries {
            let mut entry = entry.map_err(|source_err| {
                FsOpsError::io("extract_tar.read_entry", source, source_err)
            })?;
            let entry_path = entry.path().map_err(|source_err| {
                FsOpsError::io("extract_tar.entry_path", source, source_err)
            })?;
            let entry_path = Self::sanitize_path_components(&entry_path, "archive_entry")?;
            let destination = target.join(&entry_path);

            if entry.header().entry_type().is_dir() {
                Self::create_directory(&destination, TAR_EXTRACT_OPS.create_dir)?;
                continue;
            }

            if !entry.header().entry_type().is_file() {
                warn!(
                    path = %entry_path.display(),
                    "skipping unsupported tar entry type during extraction"
                );
                continue;
            }

            let unix_mode = entry.header().mode().ok();
            Self::write_archive_file(&mut entry, &destination, TAR_EXTRACT_OPS, unix_mode)?;
        }

        Ok(())
    }

    fn create_directory(path: &Path, operation: &'static str) -> FsOpsResult<()> {
        fs::create_dir_all(path).map_err(|source_err| FsOpsError::io(operation, path, source_err))
    }

    fn ensure_parent_directory(path: &Path, operation: &'static str) -> FsOpsResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|source_err| FsOpsError::io(operation, parent, source_err))?;
        }
        Ok(())
    }

    fn write_archive_file<R: Read>(
        reader: &mut R,
        destination: &Path,
        ops: ArchiveWriteOps,
        unix_mode: Option<u32>,
    ) -> FsOpsResult<()> {
        Self::ensure_parent_directory(destination, ops.create_parent)?;
        let mut output = File::create(destination)
            .map_err(|source_err| FsOpsError::io(ops.create_file, destination, source_err))?;
        io::copy(reader, &mut output)
            .map_err(|source_err| FsOpsError::io(ops.copy, destination, source_err))?;

        #[cfg(unix)]
        if let Some(mode) = unix_mode {
            let perms = fs::Permissions::from_mode(mode);
            fs::set_permissions(destination, perms).map_err(|source_err| {
                FsOpsError::io(ops.set_permissions, destination, source_err)
            })?;
        }

        #[cfg(not(unix))]
        let _ = unix_mode;

        Ok(())
    }

    fn extract_with_external_tool(
        operation: &'static str,
        candidates: &[ExternalCommand],
    ) -> FsOpsResult<()> {
        let _program = Self::run_external_command(operation, None, candidates)?;
        Ok(())
    }

    fn run_par2_tool(mode: Par2Mode, working_dir: &Path, par2_file: &Path) -> FsOpsResult<String> {
        let program = Self::run_external_command(
            "par2",
            Some(working_dir),
            &[ExternalCommand {
                program: "par2",
                args: vec![
                    mode.command().to_string(),
                    par2_file.to_string_lossy().into_owned(),
                ],
            }],
        )?;
        Ok(format!(
            "mode={} tool={} file={}",
            mode.label(),
            program,
            par2_file.display()
        ))
    }

    fn run_external_command(
        operation: &'static str,
        working_dir: Option<&Path>,
        candidates: &[ExternalCommand],
    ) -> FsOpsResult<String> {
        let mut attempted_missing = Vec::new();
        let mut last_process_error = None;

        for candidate in candidates {
            let resolved_program = Self::resolve_external_program(candidate.program);
            let mut command = Command::new(&resolved_program);
            command.args(&candidate.args);
            if let Some(dir) = working_dir {
                command.current_dir(dir);
            }

            match command.output() {
                Ok(output) if output.status.success() => {
                    return Ok(candidate.program.to_string());
                }
                Ok(output) => {
                    last_process_error = Some(FsOpsError::Process {
                        operation,
                        program: candidate.program.to_string(),
                        status: output.status.code(),
                        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
                    });
                }
                Err(source_err) if source_err.kind() == io::ErrorKind::NotFound => {
                    attempted_missing.push(candidate.program.to_string());
                }
                Err(source_err) => {
                    return Err(FsOpsError::io(
                        operation,
                        PathBuf::from(&resolved_program),
                        source_err,
                    ));
                }
            }
        }

        if let Some(error) = last_process_error {
            return Err(error);
        }

        Err(FsOpsError::MissingTool {
            operation,
            programs: attempted_missing,
        })
    }

    #[cfg(not(test))]
    fn resolve_external_program(program: &str) -> String {
        program.to_string()
    }

    #[cfg(test)]
    fn resolve_external_program(program: &str) -> String {
        let overrides = external_tool_overrides();
        let guard = match overrides.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.get(program).map_or_else(
            || program.to_string(),
            |path| path.to_string_lossy().into_owned(),
        )
    }

    fn find_par2_file(search_root: &Path, source: &Path) -> FsOpsResult<Option<PathBuf>> {
        if !search_root.exists() {
            return Ok(None);
        }

        let preferred_stem = source
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_ascii_lowercase);
        let mut candidates = Vec::new();
        for entry in WalkDir::new(search_root) {
            let entry = entry
                .map_err(|source_err| FsOpsError::walkdir("par2.walk", search_root, source_err))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let is_par2 = path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("par2"));
            if is_par2 {
                candidates.push(path.to_path_buf());
            }
        }

        candidates.sort();
        let preferred = candidates.iter().find(|path| {
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_ascii_lowercase);
            let Some(file_name) = file_name else {
                return false;
            };
            let preferred_matches = preferred_stem
                .as_ref()
                .is_some_and(|stem| file_name.starts_with(stem));
            preferred_matches && !file_name.contains(".vol")
        });
        if let Some(path) = preferred {
            return Ok(Some(path.clone()));
        }

        let primary = candidates.iter().find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_ascii_lowercase)
                .is_some_and(|name| !name.contains(".vol"))
        });
        if let Some(path) = primary {
            return Ok(Some(path.clone()));
        }

        Ok(candidates.into_iter().next())
    }

    fn sanitize_archive_path(entry: &str) -> FsOpsResult<PathBuf> {
        let path = Path::new(entry);
        Self::sanitize_path_components(path, "archive_entry").map_err(|error| match error {
            FsOpsError::InvalidInput { field, reason, .. } if field == "archive_entry" => {
                FsOpsError::InvalidInput {
                    field,
                    reason,
                    value: Some(entry.to_string()),
                }
            }
            other => other,
        })
    }

    fn sanitize_path_components(path: &Path, field: &'static str) -> FsOpsResult<PathBuf> {
        if path.is_absolute() {
            return Err(FsOpsError::InvalidInput {
                field,
                reason: "absolute_path",
                value: Some(path.to_string_lossy().into_owned()),
            });
        }

        let mut sanitized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::Normal(segment) => sanitized.push(segment),
                Component::CurDir => {}
                _ => {
                    return Err(FsOpsError::InvalidInput {
                        field,
                        reason: "invalid_segment",
                        value: Some(path.to_string_lossy().into_owned()),
                    });
                }
            }
        }

        Ok(sanitized)
    }

    fn relative_path<'a>(
        root: &Path,
        path: &'a Path,
        field: &'static str,
    ) -> FsOpsResult<&'a Path> {
        path.strip_prefix(root)
            .map_err(|_| FsOpsError::InvalidInput {
                field,
                reason: "strip_prefix",
                value: Some(path.to_string_lossy().into_owned()),
            })
    }

    fn build_checksums(artifact: &Path) -> FsOpsResult<Vec<ChecksumRecord>> {
        if artifact.is_file() {
            let checksum = Self::hash_file(artifact)?;
            let file_name = artifact
                .file_name()
                .and_then(|name| name.to_str())
                .map_or_else(
                    || artifact.display().to_string(),
                    std::borrow::ToOwned::to_owned,
                );
            let manifest_line =
                Self::checksum_manifest_line(&file_name, checksum.bytes, &checksum.value);
            let (manifest_value, manifest_bytes) =
                Self::build_manifest_checksum(std::slice::from_ref(&manifest_line));
            return Ok(vec![
                checksum.with_path(file_name),
                ChecksumRecord {
                    path: ".".to_string(),
                    kind: ChecksumKind::Manifest,
                    algorithm: "sha256".to_string(),
                    value: manifest_value,
                    bytes: manifest_bytes,
                },
            ]);
        }

        let mut file_checksums = Vec::new();
        for entry in WalkDir::new(artifact) {
            let entry = entry
                .map_err(|source_err| FsOpsError::walkdir("checksum.walk", artifact, source_err))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let relative = Self::relative_path(artifact, entry.path(), "artifact_path")?;
            let relative = relative.to_string_lossy().replace('\\', "/");
            file_checksums.push(Self::hash_file(entry.path())?.with_path(relative));
        }

        file_checksums.sort_by(|left, right| left.path.cmp(&right.path));

        let manifest_lines: Vec<String> = file_checksums
            .iter()
            .map(|record| Self::checksum_manifest_line(&record.path, record.bytes, &record.value))
            .collect();
        let (manifest_value, manifest_bytes) = Self::build_manifest_checksum(&manifest_lines);
        let mut checksums = file_checksums;
        checksums.push(ChecksumRecord {
            path: ".".to_string(),
            kind: ChecksumKind::Manifest,
            algorithm: "sha256".to_string(),
            value: manifest_value,
            bytes: manifest_bytes,
        });
        Ok(checksums)
    }

    fn checksum_manifest_line(path: &str, bytes: u64, value: &str) -> String {
        format!("{path}\t{bytes}\t{value}\n")
    }

    fn build_manifest_checksum(lines: &[String]) -> (String, u64) {
        let mut manifest = Sha256::new();
        let mut manifest_bytes = 0_u64;
        for line in lines {
            manifest.update(line.as_bytes());
            manifest_bytes += line.len() as u64;
        }
        (format!("{:x}", manifest.finalize()), manifest_bytes)
    }

    fn hash_file(path: &Path) -> FsOpsResult<ChecksumRecord> {
        let mut file = File::open(path)
            .map_err(|source_err| FsOpsError::io("checksum.open", path, source_err))?;
        let mut buffer = [0_u8; 8192];
        let mut hasher = Sha256::new();
        let mut bytes = 0_u64;
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|source_err| FsOpsError::io("checksum.read", path, source_err))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            bytes += read as u64;
        }

        Ok(ChecksumRecord {
            path: String::new(),
            kind: ChecksumKind::File,
            algorithm: "sha256".to_string(),
            value: format!("{:x}", hasher.finalize()),
            bytes,
        })
    }

    fn copy_tree(source: &Path, destination: &Path) -> FsOpsResult<()> {
        Self::replicate_tree(source, destination, COPY_TREE_OPS, |from, to, operation| {
            fs::copy(from, to)
                .map(|_| ())
                .map_err(|source_err| FsOpsError::io(operation, to, source_err))
        })
    }

    fn move_tree(source: &Path, destination: &Path) -> FsOpsResult<()> {
        match fs::rename(source, destination) {
            Ok(()) => Ok(()),
            Err(_rename_err) => {
                Self::copy_tree(source, destination)?;
                if let Err(remove_err) = fs::remove_dir_all(source) {
                    if remove_err.kind() == io::ErrorKind::NotFound {
                        return Ok(());
                    }
                    if let Err(file_err) = fs::remove_file(source)
                        && file_err.kind() != io::ErrorKind::NotFound
                    {
                        return Err(FsOpsError::io("move_tree.cleanup", source, file_err));
                    }
                }
                Ok(())
            }
        }
    }

    fn hardlink_tree(source: &Path, destination: &Path) -> FsOpsResult<()> {
        Self::replicate_tree(
            source,
            destination,
            HARDLINK_TREE_OPS,
            |from, to, operation| {
                fs::hard_link(from, to)
                    .map_err(|source_err| FsOpsError::io(operation, to, source_err))
            },
        )
    }

    fn replicate_tree<F>(
        source: &Path,
        destination: &Path,
        ops: TreeTransferOps,
        mut transfer_file: F,
    ) -> FsOpsResult<()>
    where
        F: FnMut(&Path, &Path, &'static str) -> FsOpsResult<()>,
    {
        if source.is_file() {
            Self::ensure_parent_directory(destination, ops.create_parent)?;
            transfer_file(source, destination, ops.transfer_file)?;
            return Ok(());
        }

        Self::create_directory(destination, ops.create_dir)?;

        for entry in WalkDir::new(source) {
            let entry =
                entry.map_err(|source_err| FsOpsError::walkdir(ops.walk, source, source_err))?;
            let relative = Self::relative_path(source, entry.path(), "source_path")?;
            let target_path = destination.join(relative);
            if entry.file_type().is_dir() {
                Self::create_directory(&target_path, ops.create_dir)?;
            } else {
                Self::ensure_parent_directory(&target_path, ops.create_parent)?;
                transfer_file(entry.path(), &target_path, ops.transfer_entry)?;
            }
        }

        Ok(())
    }

    fn apply_permissions(
        destination: &Path,
        file_mode: Option<&str>,
        dir_mode: Option<&str>,
        owner: Option<&str>,
        group: Option<&str>,
        umask: Option<&str>,
    ) -> FsOpsResult<String> {
        #[cfg(not(unix))]
        {
            let mut requested = Vec::new();
            if file_mode.is_some() {
                requested.push("chmod_file");
            }
            if dir_mode.is_some() {
                requested.push("chmod_dir");
            }
            if owner.is_some() {
                requested.push("owner");
            }
            if group.is_some() {
                requested.push("group");
            }
            if umask.is_some() {
                requested.push("umask");
            }
            return Err(FsOpsError::Unsupported {
                operation: "apply_permissions",
                value: Some(requested.join(",")),
            });
        }

        #[cfg(unix)]
        {
            let file_spec = file_mode
                .map(|value| Self::parse_octal_mode("chmod_file", value))
                .transpose()?;
            let dir_spec = dir_mode
                .map(|value| Self::parse_octal_mode("chmod_dir", value))
                .transpose()?;
            let umask_spec = umask
                .map(|value| Self::parse_octal_mode("umask", value))
                .transpose()?;

            let file_mode = match (file_spec, umask_spec) {
                (Some(mode), _) => Some((mode, false)),
                (None, Some(mask)) => Some((0o666 & !mask, true)),
                (None, None) => None,
            };
            let dir_mode = match (dir_spec, umask_spec) {
                (Some(mode), _) => Some((mode, false)),
                (None, Some(mask)) => Some((0o777 & !mask, true)),
                (None, None) => None,
            };

            for entry in WalkDir::new(destination) {
                let entry = entry.map_err(|source_err| {
                    FsOpsError::walkdir("apply_permissions.walk", destination, source_err)
                })?;
                let path = entry.path();
                if entry.file_type().is_dir() {
                    if let Some((mode, _)) = dir_mode {
                        let perms = fs::Permissions::from_mode(mode);
                        fs::set_permissions(path, perms).map_err(|source_err| {
                            FsOpsError::io("apply_permissions.set_dir", path, source_err)
                        })?;
                    }
                } else if let Some((mode, _)) = file_mode {
                    let perms = fs::Permissions::from_mode(mode);
                    fs::set_permissions(path, perms).map_err(|source_err| {
                        FsOpsError::io("apply_permissions.set_file", path, source_err)
                    })?;
                }
            }

            let mut detail_components = Vec::new();
            if let Some((mode, _)) = file_mode {
                detail_components.push(format!("file=0o{mode:o}"));
            }
            if let Some((mode, _)) = dir_mode {
                detail_components.push(format!("dir=0o{mode:o}"));
            }
            if let Some(mask) = umask_spec
                && (file_mode.is_some_and(|(_, derived)| derived)
                    || dir_mode.is_some_and(|(_, derived)| derived))
            {
                detail_components.push(format!("umask=0o{mask:o}"));
            }

            let ownership_details = Self::apply_ownership(destination, owner, group)?;
            detail_components.extend(ownership_details);

            if detail_components.is_empty() {
                detail_components.push("unchanged".to_string());
            }
            Ok(format!("permissions={}", detail_components.join(",")))
        }
    }

    #[cfg(unix)]
    fn apply_ownership(
        destination: &Path,
        owner: Option<&str>,
        group: Option<&str>,
    ) -> FsOpsResult<Vec<String>> {
        let owner = owner.map(Self::resolve_owner).transpose()?;
        let group = group.map(Self::resolve_group).transpose()?;
        if owner.is_none() && group.is_none() {
            return Ok(Vec::new());
        }

        let uid = owner.as_ref().map(|(uid, _)| *uid);
        let gid = group.as_ref().map(|(gid, _)| *gid);

        for entry in WalkDir::new(destination) {
            let entry = entry.map_err(|source_err| {
                FsOpsError::walkdir("apply_ownership.walk", destination, source_err)
            })?;
            let path = entry.path();
            chown(path, uid, gid).map_err(|source_err| FsOpsError::Nix {
                operation: "apply_ownership.chown",
                path: path.to_path_buf(),
                source: source_err,
            })?;
        }

        let mut detail = Vec::new();
        if let Some((_, label)) = owner {
            detail.push(format!("owner={label}"));
        }
        if let Some((_, label)) = group {
            detail.push(format!("group={label}"));
        }
        Ok(detail)
    }

    #[cfg(not(unix))]
    fn apply_ownership(
        _destination: &Path,
        owner: Option<&str>,
        group: Option<&str>,
    ) -> FsOpsResult<Vec<String>> {
        if owner.is_some() || group.is_some() {
            return Err(FsOpsError::Unsupported {
                operation: "apply_ownership",
                value: Some("unix_only".to_string()),
            });
        }
        Ok(Vec::new())
    }

    #[cfg(unix)]
    fn resolve_owner(spec: &str) -> FsOpsResult<(Uid, String)> {
        let trimmed = spec.trim();
        if trimmed.is_empty() {
            return Err(FsOpsError::InvalidInput {
                field: "owner",
                reason: "empty",
                value: Some(spec.to_string()),
            });
        }
        if let Ok(id) = trimmed.parse::<u32>() {
            let uid = Uid::from_raw(id);
            return Ok((uid, format!("uid({id})")));
        }
        let user = User::from_name(trimmed)
            .map_err(|source_err| FsOpsError::UserLookup {
                user: trimmed.to_string(),
                source: source_err,
            })?
            .ok_or_else(|| FsOpsError::InvalidInput {
                field: "owner",
                reason: "not_found",
                value: Some(trimmed.to_string()),
            })?;
        Ok((user.uid, format!("{trimmed}({})", user.uid.as_raw())))
    }

    #[cfg(unix)]
    fn resolve_group(spec: &str) -> FsOpsResult<(Gid, String)> {
        let trimmed = spec.trim();
        if trimmed.is_empty() {
            return Err(FsOpsError::InvalidInput {
                field: "group",
                reason: "empty",
                value: Some(spec.to_string()),
            });
        }
        if let Ok(id) = trimmed.parse::<u32>() {
            let gid = Gid::from_raw(id);
            return Ok((gid, format!("gid({id})")));
        }
        let group = Group::from_name(trimmed)
            .map_err(|source_err| FsOpsError::GroupLookup {
                group: trimmed.to_string(),
                source: source_err,
            })?
            .ok_or_else(|| FsOpsError::InvalidInput {
                field: "group",
                reason: "not_found",
                value: Some(trimmed.to_string()),
            })?;
        Ok((group.gid, format!("{trimmed}({})", group.gid.as_raw())))
    }

    fn cleanup_destination(destination: &Path, rules: &RuleSet) -> usize {
        let mut removed = 0usize;

        let mut files = Vec::new();
        let mut directories = Vec::new();
        for entry in WalkDir::new(destination) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    warn!(
                        error = %err,
                        path = %destination.display(),
                        "failed to traverse cleanup destination"
                    );
                    continue;
                }
            };
            if entry.path() == destination {
                continue;
            }
            if entry.file_type().is_dir() {
                directories.push(entry);
            } else {
                files.push(entry);
            }
        }

        for entry in files {
            match rules.evaluate(entry.path()) {
                RuleDecision::Include => {}
                RuleDecision::Skip => match fs::remove_file(entry.path()) {
                    Ok(()) => {
                        removed += 1;
                    }
                    Err(err) => {
                        warn!(
                            error = %err,
                            path = %entry.path().display(),
                            "failed to remove cleanup file"
                        );
                    }
                },
            }
        }

        directories.sort_by_key(walkdir::DirEntry::depth);
        directories.reverse();

        for entry in directories {
            match rules.evaluate(entry.path()) {
                RuleDecision::Include => {}
                RuleDecision::Skip => {
                    let is_empty = match entry.path().read_dir() {
                        Ok(mut iter) => iter.next().is_none(),
                        Err(err) => {
                            warn!(
                                error = %err,
                                path = %entry.path().display(),
                                "failed to read cleanup directory"
                            );
                            false
                        }
                    };
                    if is_empty && let Err(err) = fs::remove_dir(entry.path()) {
                        warn!(
                            error = %err,
                            path = %entry.path().display(),
                            "failed to remove cleanup directory"
                        );
                    }
                }
            }
        }

        removed
    }

    fn parse_octal_mode(field: &'static str, value: &str) -> FsOpsResult<u32> {
        let trimmed = value.trim_start_matches("0o");
        u32::from_str_radix(trimmed, 8).map_err(|_| FsOpsError::InvalidInput {
            field,
            reason: "invalid_octal",
            value: Some(value.to_string()),
        })
    }

    fn mark_degraded(&self, detail: &str) {
        let mut guard = self.lock_health_flag();
        if *guard {
            drop(guard);
            warn!(
                component = HEALTH_COMPONENT,
                detail = detail,
                "fsops pipeline still degraded"
            );
        } else {
            *guard = true;
            drop(guard);
            warn!(
                component = HEALTH_COMPONENT,
                detail = detail,
                "fsops pipeline degraded"
            );
            self.publish_event(Event::HealthChanged {
                degraded: vec![HEALTH_COMPONENT.to_string()],
            });
        }
    }

    fn mark_recovered(&self) {
        let mut guard = self.lock_health_flag();
        if std::mem::take(&mut *guard) {
            drop(guard);
            self.publish_event(Event::HealthChanged { degraded: vec![] });
            info!(component = HEALTH_COMPONENT, "fsops pipeline recovered");
        }
    }

    fn record_job_started(&self, torrent_id: Uuid, source: &Path) {
        if let Some(store) = self.runtime.clone() {
            let source_path = PathBuf::from(source);
            tokio::spawn(async move {
                if let Err(err) = store
                    .mark_fs_job_started(torrent_id, source_path.as_path())
                    .await
                {
                    warn!(
                        error = %err,
                        torrent_id = %torrent_id,
                        "failed to record fs job start"
                    );
                }
            });
        }
    }

    fn record_job_completed(&self, torrent_id: Uuid, meta: &FsOpsMeta) {
        if let Some(store) = self.runtime.clone() {
            let transfer_mode = meta.transfer_mode.clone();
            let destination = meta
                .artifact_path
                .as_ref()
                .or(meta.staging_path.as_ref())
                .or(meta.source_path.as_ref())
                .map(PathBuf::from);
            let source = meta.source_path.as_ref().map(PathBuf::from);

            match (destination, source) {
                (Some(destination), Some(source)) => {
                    tokio::spawn(async move {
                        if let Err(err) = store
                            .mark_fs_job_completed(
                                torrent_id,
                                source.as_path(),
                                destination.as_path(),
                                transfer_mode.as_deref(),
                            )
                            .await
                        {
                            warn!(
                                error = %err,
                                torrent_id = %torrent_id,
                                "failed to record fs job completion"
                            );
                        }
                    });
                }
                (Some(_), None) => {
                    tokio::spawn(async move {
                        if let Err(err) = store
                            .mark_fs_job_failed(
                                torrent_id,
                                "fsops completed without recorded source path",
                            )
                            .await
                        {
                            warn!(
                                error = %err,
                                torrent_id = %torrent_id,
                                "failed to record fs job completion fallback without source"
                            );
                        }
                    });
                }
                (None, _) => {
                    tokio::spawn(async move {
                        if let Err(err) = store
                            .mark_fs_job_failed(torrent_id, "fsops completed without artifact")
                            .await
                        {
                            warn!(
                                error = %err,
                                torrent_id = %torrent_id,
                                "failed to record fs job completion fallback"
                            );
                        }
                    });
                }
            }
        }
    }

    fn publish_event(&self, event: Event) {
        if let Err(error) = self.events.publish(event) {
            warn!(
                event_id = error.event_id(),
                event_kind = error.event_kind(),
                error = %error,
                "failed to publish event"
            );
        }
    }

    fn record_job_failed(&self, torrent_id: Uuid, source: &Path, message: String) {
        if let Some(store) = self.runtime.clone() {
            let source_path = PathBuf::from(source);
            tokio::spawn(async move {
                let has_state = match store.fetch_fs_job_state(torrent_id).await {
                    Ok(state) => state.is_some(),
                    Err(err) => {
                        warn!(
                            error = %err,
                            torrent_id = %torrent_id,
                            "failed to inspect fs job state before recording failure"
                        );
                        false
                    }
                };
                if !has_state
                    && let Err(err) = store
                        .mark_fs_job_started(torrent_id, source_path.as_path())
                        .await
                {
                    warn!(
                        error = %err,
                        torrent_id = %torrent_id,
                        "failed to seed fs job state before recording failure"
                    );
                }
                if let Err(err) = store.mark_fs_job_failed(torrent_id, &message).await {
                    warn!(
                        error = %err,
                        torrent_id = %torrent_id,
                        "failed to record fs job failure"
                    );
                }
            });
        }
    }

    fn lock_health_flag(&self) -> MutexGuard<'_, bool> {
        match self.health_degraded.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                error!("fsops health mutex poisoned; continuing with recovered guard");
                poisoned.into_inner()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FsOpsMeta {
    torrent_id: Uuid,
    policy_id: Uuid,
    completed: bool,
    updated_at: DateTime<Utc>,
    steps: Vec<StepRecord>,
    source_path: Option<String>,
    work_dir: Option<String>,
    staging_path: Option<String>,
    artifact_path: Option<String>,
    transfer_mode: Option<String>,
    #[serde(default)]
    checksums: Vec<ChecksumRecord>,
}

impl FsOpsMeta {
    fn new(torrent_id: Uuid, policy_id: Uuid) -> Self {
        Self {
            torrent_id,
            policy_id,
            completed: false,
            updated_at: Utc::now(),
            steps: Vec::new(),
            source_path: None,
            work_dir: None,
            staging_path: None,
            artifact_path: None,
            transfer_mode: None,
            checksums: Vec::new(),
        }
    }

    fn step_status(&self, step: StepKind) -> Option<StepStatus> {
        self.steps
            .iter()
            .find(|record| record.name == step.as_str())
            .map(|record| record.status)
    }

    fn update_step(&mut self, step: StepKind, status: StepStatus, detail: Option<String>) -> bool {
        let now = Utc::now();
        let mut updated = false;
        if let Some(record) = self
            .steps
            .iter_mut()
            .find(|record| record.name == step.as_str())
        {
            let detail_changed = detail != record.detail;
            if record.status != status || detail_changed {
                record.status = status;
                record.detail = detail;
                record.updated_at = now;
                updated = true;
            }
        } else {
            self.steps.push(StepRecord {
                name: step.as_str().to_string(),
                status,
                detail,
                updated_at: now,
            });
            updated = true;
        }
        if updated {
            self.updated_at = now;
        }
        updated
    }
}

fn load_meta(path: &Path) -> FsOpsResult<FsOpsMeta> {
    let raw = fs::read_to_string(path)
        .map_err(|source_err| FsOpsError::io("meta.read", path, source_err))?;
    serde_json::from_str(&raw)
        .map_err(|source_err| FsOpsError::json("meta.parse", path, source_err))
}

fn persist_meta(path: &Path, meta: &FsOpsMeta) -> FsOpsResult<()> {
    let serialised = serde_json::to_string_pretty(meta)
        .map_err(|source_err| FsOpsError::json("meta.serialize", path, source_err))?;
    fs::write(path, serialised).map_err(|source_err| FsOpsError::io("meta.write", path, source_err))
}

fn enforce_allow_paths(root: &Path, allow_paths: &[String]) -> FsOpsResult<()> {
    let allows = parse_path_list(allow_paths)?;
    if allows.is_empty() {
        return Ok(());
    }

    if allows.iter().any(|allow| root.starts_with(allow)) {
        return Ok(());
    }

    let root_abs = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

    let mut permitted = false;
    for allow in &allows {
        let allow_abs = allow.canonicalize().unwrap_or_else(|_| allow.clone());
        if root_abs.starts_with(&allow_abs) {
            permitted = true;
            break;
        }
    }

    if !permitted {
        return Err(FsOpsError::InvalidPolicy {
            field: "allow_paths",
            reason: "root_not_permitted",
            value: Some(root_abs.to_string_lossy().into_owned()),
        });
    }

    Ok(())
}

fn parse_path_list(entries: &[String]) -> FsOpsResult<Vec<PathBuf>> {
    entries
        .iter()
        .map(|entry| {
            if entry.trim().is_empty() {
                Err(FsOpsError::InvalidPolicy {
                    field: "allow_paths",
                    reason: "empty_entry",
                    value: Some(entry.clone()),
                })
            } else {
                Ok(PathBuf::from(entry))
            }
        })
        .collect()
}

#[derive(Debug)]
struct RuleSet {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
}

impl RuleSet {
    fn from_policy(policy: &FsPolicy) -> FsOpsResult<Self> {
        let include_patterns = parse_glob_list(&policy.cleanup_keep, "cleanup_keep")?;
        let mut exclude_patterns = parse_glob_list(&policy.cleanup_drop, "cleanup_drop")?;

        if exclude_patterns
            .iter()
            .any(|pattern| pattern == SKIP_FLUFF_PRESET)
        {
            exclude_patterns.retain(|pattern| pattern != SKIP_FLUFF_PRESET);
            exclude_patterns.extend(
                SKIP_FLUFF_PATTERNS
                    .iter()
                    .map(std::string::ToString::to_string),
            );
        }

        Ok(Self {
            include: build_globset(include_patterns, "cleanup_keep")?,
            exclude: build_globset(exclude_patterns, "cleanup_drop")?,
        })
    }

    fn evaluate(&self, path: &Path) -> RuleDecision {
        if self
            .exclude
            .as_ref()
            .is_some_and(|exclude| exclude.is_match(path))
        {
            return RuleDecision::Skip;
        }

        match &self.include {
            Some(include) if include.is_match(path) => RuleDecision::Include,
            Some(_) => RuleDecision::Skip,
            None => RuleDecision::Include,
        }
    }

    fn include_count(&self) -> usize {
        self.include.as_ref().map_or(0, globset::GlobSet::len)
    }

    fn exclude_count(&self) -> usize {
        self.exclude.as_ref().map_or(0, globset::GlobSet::len)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum RuleDecision {
    Include,
    Skip,
}

fn parse_glob_list(entries: &[String], field: &'static str) -> FsOpsResult<Vec<String>> {
    entries
        .iter()
        .map(|pattern| {
            if pattern.trim().is_empty() {
                Err(FsOpsError::InvalidPolicy {
                    field,
                    reason: "empty_pattern",
                    value: Some(pattern.clone()),
                })
            } else {
                Ok(pattern.clone())
            }
        })
        .collect()
}

fn build_globset(patterns: Vec<String>, field: &'static str) -> FsOpsResult<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(
            Glob::new(&pattern)
                .map_err(|source_err| FsOpsError::glob(field, pattern.clone(), source_err))?,
        );
    }
    Ok(Some(builder.build().map_err(|source_err| {
        FsOpsError::glob(field, "<set>".to_string(), source_err)
    })?))
}

#[cfg(test)]
fn external_tool_overrides() -> &'static Mutex<HashMap<String, PathBuf>> {
    use std::sync::OnceLock;

    static OVERRIDES: OnceLock<Mutex<HashMap<String, PathBuf>>> = OnceLock::new();
    OVERRIDES.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use revaer_events::TorrentState;
    use revaer_runtime::RuntimeStore as PersistedRuntimeStore;
    use revaer_test_support::postgres::{TestDatabase, start_postgres};
    use revaer_torrent_core::{TorrentProgress, TorrentRates, TorrentStatus};
    use sqlx::postgres::PgPoolOptions;
    use std::cell::Cell;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::runtime::Runtime;
    use tokio::time::{Duration, sleep};
    use tokio_stream::StreamExt;

    type TestResult<T> = Result<T>;

    fn repo_root() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for ancestor in manifest_dir.ancestors() {
            if ancestor.join("AGENT.md").is_file() {
                return ancestor.to_path_buf();
            }
        }
        manifest_dir
    }

    fn server_root() -> TestResult<PathBuf> {
        let root = repo_root().join(".server_root");
        fs::create_dir_all(&root)?;
        Ok(root)
    }

    fn temp_dir() -> TestResult<TempDir> {
        Ok(tempfile::Builder::new()
            .prefix("revaer-fsops-")
            .tempdir_in(server_root()?)?)
    }

    #[cfg(unix)]
    fn write_executable(path: &Path, contents: &str) -> TestResult<()> {
        use std::os::unix::fs::PermissionsExt;

        fs::write(path, contents)?;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
        Ok(())
    }

    fn set_tool_override(program: &str, path: &Path) {
        let overrides = external_tool_overrides();
        let mut guard = match overrides.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.insert(program.to_string(), path.to_path_buf());
    }

    fn clear_tool_overrides() {
        let overrides = external_tool_overrides();
        let mut guard = match overrides.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.clear();
    }

    fn external_tool_test_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    fn sample_policy(root: &Path) -> FsPolicy {
        FsPolicy {
            id: Uuid::new_v4(),
            library_root: root.join("library").display().to_string(),
            extract: false,
            par2: "disabled".to_string(),
            flatten: false,
            move_mode: "copy".to_string(),
            cleanup_keep: vec!["**/*.mkv".to_string()],
            cleanup_drop: Vec::new(),
            chmod_file: None,
            chmod_dir: None,
            owner: None,
            group: None,
            umask: None,
            allow_paths: vec![root.display().to_string()],
        }
    }

    fn write_tar_archive(archive: &Path, entries: &[(&str, &[u8])]) -> TestResult<()> {
        let file = File::create(archive)?;
        let mut builder = tar::Builder::new(file);
        for (path, contents) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_path(path)?;
            header.set_mode(0o644);
            header.set_size(contents.len() as u64);
            header.set_cksum();
            builder.append(&header, *contents)?;
        }
        builder.finish()?;
        Ok(())
    }

    fn write_tgz_archive(archive: &Path, entries: &[(&str, &[u8])]) -> TestResult<()> {
        let file = File::create(archive)?;
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for (path, contents) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_path(path)?;
            header.set_mode(0o644);
            header.set_size(contents.len() as u64);
            header.set_cksum();
            builder.append(&header, *contents)?;
        }
        builder.finish()?;
        Ok(())
    }

    async fn runtime_store() -> TestResult<Option<(TestDatabase, PersistedRuntimeStore)>> {
        let postgres = match start_postgres() {
            Ok(database) => database,
            Err(err) => {
                eprintln!("skipping fsops runtime test: {err}");
                return Ok(None);
            }
        };
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(postgres.connection_string())
            .await?;
        let store = PersistedRuntimeStore::new(pool).await?;
        Ok(Some((postgres, store)))
    }

    fn sample_status(torrent_id: Uuid) -> TorrentStatus {
        TorrentStatus {
            id: torrent_id,
            name: Some("fsops-runtime".to_string()),
            state: TorrentState::Queued,
            progress: TorrentProgress {
                bytes_downloaded: 0,
                bytes_total: 0,
                eta_seconds: None,
            },
            rates: TorrentRates {
                download_bps: 0,
                upload_bps: 0,
                ratio: 0.0,
            },
            files: None,
            library_path: Some(".server_root/library/fsops-runtime".to_string()),
            download_dir: Some(".server_root/downloads".to_string()),
            comment: None,
            source: Some("integration".to_string()),
            private: Some(false),
            sequential: false,
            added_at: Utc::now(),
            completed_at: None,
            last_updated: Utc::now(),
        }
    }

    async fn seed_runtime_torrent(
        store: &PersistedRuntimeStore,
        torrent_id: Uuid,
    ) -> TestResult<()> {
        store.upsert_status(&sample_status(torrent_id)).await?;
        Ok(())
    }

    async fn wait_for_fs_job_state(
        store: &PersistedRuntimeStore,
        torrent_id: Uuid,
    ) -> TestResult<
        Option<(
            String,
            i16,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        )>,
    > {
        for _ in 0..20 {
            if let Some(state) = store.fetch_fs_job_state(torrent_id).await? {
                let terminal = state.status != "moving"
                    || state.dst_path.is_some()
                    || state.last_error.is_some();
                if terminal {
                    return Ok(Some((
                        state.status,
                        state.attempt,
                        state.src_path,
                        state.dst_path,
                        state.transfer_mode,
                        state.last_error,
                    )));
                }
            }
            sleep(Duration::from_millis(50)).await;
        }
        Ok(None)
    }

    #[test]
    fn build_glob_set_matches_expected_paths() -> TestResult<()> {
        let policy = sample_policy(Path::new(".server_root"));
        let patterns = parse_glob_list(&policy.cleanup_keep, "cleanup_keep")?;
        let glob_rules = build_globset(patterns, "cleanup_keep")?;
        let glob_set = glob_rules.ok_or(FsOpsError::MissingState {
            field: "glob_rules",
        })?;

        assert!(glob_set.is_match(".server_root/library/movie/file.mkv"));
        assert!(!glob_set.is_match(".server_root/library/movie/file.srt"));
        assert!(!glob_set.is_match(".server_root/library/movie/file.txt"));

        Ok(())
    }

    #[test]
    fn rule_set_evaluates_include_and_exclude() -> TestResult<()> {
        let policy = FsPolicy {
            id: Uuid::new_v4(),
            library_root: ".server_root/library".to_string(),
            extract: false,
            par2: "disabled".to_string(),
            flatten: false,
            move_mode: "copy".to_string(),
            cleanup_keep: vec!["**/*.mkv".to_string()],
            cleanup_drop: vec!["**/extras/**".to_string()],
            chmod_file: None,
            chmod_dir: None,
            owner: None,
            group: None,
            umask: None,
            allow_paths: Vec::new(),
        };

        let rules = RuleSet::from_policy(&policy)?;
        assert_eq!(
            rules.evaluate(Path::new("show/season1/episode1.mkv")),
            RuleDecision::Include
        );
        assert_eq!(
            rules.evaluate(Path::new("show/extras/bloopers.mp4")),
            RuleDecision::Skip
        );
        assert_eq!(
            rules.evaluate(Path::new("show/season1/episode1.mp4")),
            RuleDecision::Skip
        );
        Ok(())
    }

    #[test]
    fn parse_path_list_rejects_invalid_entries() -> TestResult<()> {
        let values = vec![String::new(), ".server_root/tmp".to_string()];
        let err = parse_path_list(&values)
            .err()
            .ok_or(FsOpsError::MissingState {
                field: "expected_invalid_path_list_error",
            })?;
        assert!(matches!(
            err,
            FsOpsError::InvalidPolicy {
                field: "allow_paths",
                reason: "empty_entry",
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn parse_glob_list_rejects_non_strings() -> TestResult<()> {
        let values = vec![String::new()];
        let err =
            parse_glob_list(&values, "cleanup_keep")
                .err()
                .ok_or(FsOpsError::MissingState {
                    field: "expected_empty_glob_error",
                })?;
        assert!(matches!(
            err,
            FsOpsError::InvalidPolicy {
                field: "cleanup_keep",
                reason: "empty_pattern",
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn build_globset_rejects_invalid_pattern() -> TestResult<()> {
        let err = build_globset(vec!["[]".to_string()], "cleanup_drop")
            .err()
            .ok_or(FsOpsError::MissingState {
                field: "expected_invalid_glob_error",
            })?;
        assert!(matches!(err, FsOpsError::Glob { .. }));
        Ok(())
    }

    fn write_zip_archive(archive: &Path, entries: &[(&str, &[u8])]) -> TestResult<()> {
        let file = File::create(archive)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default();
        for (path, contents) in entries {
            zip.start_file(*path, options)?;
            zip.write_all(contents)?;
        }
        zip.finish()?;
        Ok(())
    }

    #[test]
    fn prepare_directories_fails_for_file_path() -> TestResult<()> {
        let events = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(events, metrics);

        let temp = temp_dir()?;
        let file_root = temp.path().join("not_a_dir");
        fs::write(&file_root, "file")?;
        let meta_dir = temp.path().join("meta");
        let meta_path = temp.path().join("meta.json");

        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());

        let result = service
            .run_prepare_directories(torrent_id, &mut meta, &meta_path, &file_root, &meta_dir);

        assert!(
            result.is_err(),
            "expected directory creation to fail on file path"
        );
        Ok(())
    }

    #[test]
    fn prepare_work_dir_fails_for_file_target() -> TestResult<()> {
        let events = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(events, metrics);

        let temp = temp_dir()?;
        let meta_dir = temp.path().join("meta");
        fs::create_dir_all(&meta_dir)?;
        let work_file = meta_dir.join("work-file");
        fs::write(&work_file, "file")?;
        let meta_path = temp.path().join("meta.json");

        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        meta.work_dir = Some(work_file.to_string_lossy().into_owned());

        let err = service
            .run_prepare_work_dir(torrent_id, &mut meta, &meta_path, &meta_dir)
            .err()
            .ok_or(FsOpsError::MissingState {
                field: "expected_work_dir_error",
            })?;

        assert!(matches!(
            err,
            FsOpsError::Io {
                operation: "prepare_work_dir.create_dir",
                ..
            }
        ));
        assert_eq!(
            meta.step_status(StepKind::PrepareWorkDir),
            Some(StepStatus::Failed)
        );
        Ok(())
    }

    #[test]
    fn validate_policy_rejects_empty_library_root() -> TestResult<()> {
        let events = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(events, metrics);
        let temp = temp_dir()?;
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");
        let mut policy = sample_policy(temp.path());
        policy.library_root = "   ".to_string();

        let err = service
            .run_validate_policy(torrent_id, &mut meta, &meta_path, &policy)
            .expect_err("expected empty library_root to be rejected");

        assert!(matches!(
            err,
            FsOpsError::InvalidPolicy {
                field: "library_root",
                reason: "empty",
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn load_or_initialise_meta_populates_missing_paths() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let policy_id = Uuid::new_v4();
        let source = temp.path().join("source");
        fs::create_dir_all(&source)?;
        let meta_path = temp.path().join("meta.json");

        let meta = service.load_or_initialise_meta(torrent_id, policy_id, &meta_path, &source)?;

        let canonical = source.canonicalize().unwrap_or_else(|_| source.clone());
        let expected = canonical.to_string_lossy().into_owned();
        assert_eq!(meta.source_path.as_deref(), Some(expected.as_str()));
        assert_eq!(meta.staging_path.as_deref(), Some(expected.as_str()));
        Ok(())
    }

    #[test]
    fn load_or_initialise_meta_respects_existing_source() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let policy_id = Uuid::new_v4();
        let meta_path = temp.path().join("meta.json");

        let pinned_source = temp.path().join("pinned");
        fs::create_dir_all(&pinned_source)?;
        let mut meta = FsOpsMeta::new(torrent_id, policy_id);
        meta.source_path = Some(pinned_source.to_string_lossy().into_owned());
        persist_meta(&meta_path, &meta)?;

        let provided_source = temp.path().join("provided");
        fs::create_dir_all(&provided_source)?;

        let loaded =
            service.load_or_initialise_meta(torrent_id, policy_id, &meta_path, &provided_source)?;

        assert_eq!(loaded.source_path, meta.source_path);
        assert_eq!(loaded.staging_path, meta.source_path);
        Ok(())
    }

    #[tokio::test]
    async fn pipeline_flattens_single_directory() -> TestResult<()> {
        let bus = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let temp = temp_dir()?;

        let staging_root = temp.path().join("staging");
        fs::create_dir_all(&staging_root)?;
        let source_dir = staging_root.join("outer");
        let inner = source_dir.join("Season1");
        fs::create_dir_all(&inner)?;
        fs::write(inner.join("episode.mkv"), b"video")?;

        let mut policy = sample_policy(temp.path());
        policy.flatten = true;

        service.apply(FsOpsRequest {
            torrent_id,
            source_path: &source_dir,
            policy: &policy,
        })?;

        let meta_path = Path::new(&policy.library_root)
            .join(META_DIR_NAME)
            .join(format!("{torrent_id}{META_SUFFIX}"));
        let meta = load_meta(&meta_path)?;
        let artifact_path = meta
            .artifact_path
            .as_ref()
            .ok_or(FsOpsError::MissingState {
                field: "artifact_path",
            })?;
        let artifact = PathBuf::from(artifact_path);
        assert!(artifact.ends_with("Season1"));
        assert!(artifact.join("episode.mkv").exists());

        Ok(())
    }

    #[test]
    fn enforce_allow_paths_rejects_unpermitted_root() -> TestResult<()> {
        let temp = temp_dir()?;
        let root = temp.path().join("library");
        let allow = vec![temp.path().join("other").display().to_string()];

        let err = enforce_allow_paths(&root, &allow)
            .err()
            .ok_or(FsOpsError::MissingState {
                field: "expected_allowlist_error",
            })?;
        assert!(matches!(
            err,
            FsOpsError::InvalidPolicy {
                field: "allow_paths",
                reason: "root_not_permitted",
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn pipeline_extracts_archive_and_cleans_junk() -> TestResult<()> {
        let bus = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus.clone(), metrics);
        let mut stream = bus.subscribe(None);
        let torrent_id = Uuid::new_v4();
        let temp = temp_dir()?;

        let library_root = temp.path().join("library");
        let staging_root = temp.path().join("staging");
        fs::create_dir_all(&staging_root)?;
        let archive_path = staging_root.join("payload.zip");
        write_zip_archive(
            &archive_path,
            &[
                ("show/Season1/episode1.mkv", b"video"),
                ("show/Season1/readme.txt", b"junk"),
            ],
        )?;

        let mut policy = sample_policy(temp.path());
        policy.extract = true;
        policy.flatten = true;
        policy.cleanup_drop = vec!["**/*.txt".to_string()];
        policy.allow_paths = vec![temp.path().display().to_string()];

        service.apply(FsOpsRequest {
            torrent_id,
            source_path: &archive_path,
            policy: &policy,
        })?;

        let meta_path = library_root
            .join(META_DIR_NAME)
            .join(format!("{torrent_id}{META_SUFFIX}"));
        let meta = load_meta(&meta_path)?;
        let artifact_path = meta.artifact_path.ok_or(FsOpsError::MissingState {
            field: "artifact_path",
        })?;
        let artifact_dir = PathBuf::from(artifact_path);
        assert!(artifact_dir.exists());
        assert!(
            artifact_dir.join("Season1").join("episode1.mkv").exists(),
            "extracted artifact should preserve nested structure after flattening"
        );
        assert!(
            !artifact_dir.join("readme.txt").exists(),
            "cleanup_drop should remove junk files"
        );
        assert!(
            meta.checksums
                .iter()
                .any(|record| matches!(record.kind, ChecksumKind::Manifest)),
            "pipeline should persist checksum metadata"
        );

        // Ensure a completion event was emitted to close the stream.
        let runtime = Runtime::new()?;
        let _ = runtime.block_on(async { stream.next().await });
        Ok(())
    }

    #[test]
    fn extract_archive_rejects_missing_extension() -> TestResult<()> {
        let temp = temp_dir()?;
        let source = temp.path().join("payload");
        fs::write(&source, b"data")?;
        let target = temp.path().join("target");

        let err = FsOpsService::extract_archive(&source, &target)
            .err()
            .ok_or(FsOpsError::MissingState {
                field: "expected_missing_extension_error",
            })?;
        assert!(matches!(
            err,
            FsOpsError::InvalidInput {
                field: "archive_extension",
                reason: "missing",
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn extract_archive_supports_tar() -> TestResult<()> {
        let temp = temp_dir()?;
        let source = temp.path().join("payload.tar");
        write_tar_archive(&source, &[("show/episode1.mkv", b"video")])?;
        let target = temp.path().join("target");

        FsOpsService::extract_archive(&source, &target)?;

        assert!(target.join("show").join("episode1.mkv").exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn extract_archive_skips_tar_symlink_entries() -> TestResult<()> {
        let temp = temp_dir()?;
        let source = temp.path().join("payload.tar");
        let file = File::create(&source)?;
        let mut builder = tar::Builder::new(file);

        let mut file_header = tar::Header::new_gnu();
        file_header.set_path("show/episode1.mkv")?;
        file_header.set_size(5);
        file_header.set_mode(0o644);
        file_header.set_cksum();
        builder.append(&file_header, &b"video"[..])?;

        let mut symlink_header = tar::Header::new_gnu();
        symlink_header.set_entry_type(tar::EntryType::Symlink);
        symlink_header.set_path("show/latest.mkv")?;
        symlink_header.set_link_name("show/episode1.mkv")?;
        symlink_header.set_size(0);
        symlink_header.set_mode(0o644);
        symlink_header.set_cksum();
        builder.append(&symlink_header, io::empty())?;
        builder.finish()?;

        let target = temp.path().join("target");
        FsOpsService::extract_archive(&source, &target)?;

        assert!(target.join("show").join("episode1.mkv").exists());
        assert!(!target.join("show").join("latest.mkv").exists());
        Ok(())
    }

    #[test]
    fn extract_archive_supports_tar_gz() -> TestResult<()> {
        let temp = temp_dir()?;
        let source = temp.path().join("payload.tar.gz");
        write_tgz_archive(&source, &[("show/episode1.mkv", b"video")])?;
        let target = temp.path().join("target");

        FsOpsService::extract_archive(&source, &target)?;

        assert!(target.join("show").join("episode1.mkv").exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn extract_archive_uses_external_tool_for_7z_and_rar() -> TestResult<()> {
        let _lock = match external_tool_test_lock().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let temp = temp_dir()?;
        let log_path = temp.path().join("extract.log");
        let script = temp.path().join("7zz");
        write_executable(
            &script,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\ntarget=''\nlast=''\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    -o*) target=\"${{arg#-o}}\" ;;\n  esac\n  last=\"$arg\"\ndone\nif [ -z \"$target\" ]; then\n  case \"$last\" in\n    *.7z|*.rar|'') ;;\n    *) target=\"$last\" ;;\n  esac\nfi\nif [ -n \"$target\" ]; then\n  mkdir -p \"$target\"\n  printf 'ok' > \"$target/extracted.txt\"\nfi\n",
                log_path.display()
            ),
        )?;
        set_tool_override("7zz", &script);

        let seven_zip = temp.path().join("payload.7z");
        let rar = temp.path().join("payload.rar");
        fs::write(&seven_zip, b"7z")?;
        fs::write(&rar, b"rar")?;
        let seven_target = temp.path().join("target-7z");
        let rar_target = temp.path().join("target-rar");

        let result = (|| -> TestResult<()> {
            FsOpsService::extract_archive(&seven_zip, &seven_target)?;
            FsOpsService::extract_archive(&rar, &rar_target)?;
            Ok(())
        })();

        clear_tool_overrides();

        result?;

        let log = fs::read_to_string(&log_path)?;
        assert!(log.contains("payload.7z"));
        assert!(log.contains("payload.rar"));
        assert!(seven_target.join("extracted.txt").exists());
        assert!(rar_target.join("extracted.txt").exists());
        Ok(())
    }

    #[test]
    fn enforce_allow_paths_accepts_parent_directory() -> TestResult<()> {
        let temp = temp_dir()?;
        let root = temp.path().join("library");
        let allow = vec![temp.path().display().to_string()];
        enforce_allow_paths(&root, &allow)?;
        Ok(())
    }

    #[test]
    fn rule_set_expands_skip_fluff_preset() -> TestResult<()> {
        let mut policy = sample_policy(Path::new(".server_root"));
        policy.cleanup_drop = vec![SKIP_FLUFF_PRESET.to_string()];

        let rules = RuleSet::from_policy(&policy)?;
        assert!(rules.exclude_count() >= SKIP_FLUFF_PATTERNS.len());
        Ok(())
    }

    #[test]
    fn extract_archive_rejects_unknown_extensions() -> TestResult<()> {
        let temp = temp_dir()?;
        let source = temp.path().join("payload.iso");
        fs::write(&source, b"junk")?;
        let target = temp.path().join("target");

        let err = FsOpsService::extract_archive(&source, &target)
            .err()
            .ok_or(FsOpsError::MissingState {
                field: "expected_unsupported_archive_error",
            })?;
        assert!(matches!(
            err,
            FsOpsError::Unsupported {
                operation: "extract_archive",
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn execute_step_noops_when_already_completed() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");
        meta.update_step(StepKind::Cleanup, StepStatus::Completed, None);

        let called = Cell::new(false);
        service.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::Cleanup,
            StepPersistence::new(true, true, true),
            |_| {
                called.set(true);
                Ok(StepOutcome::Completed(None))
            },
        )?;

        assert!(!called.get(), "expected completed step to skip execution");
        Ok(())
    }

    #[test]
    fn execute_step_records_failure_status() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");

        let result = service.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::ValidatePolicy,
            StepPersistence::new(true, true, true),
            |_meta| {
                Err(FsOpsError::InvalidInput {
                    field: "test_step",
                    reason: "boom",
                    value: None,
                })
            },
        );
        assert!(result.is_err());
        let persisted = load_meta(&meta_path)?;
        assert_eq!(
            persisted.step_status(StepKind::ValidatePolicy),
            Some(StepStatus::Failed)
        );
        Ok(())
    }

    #[test]
    fn run_extract_skips_when_staging_is_directory() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");

        let staging = temp.path().join("staging");
        fs::create_dir_all(&staging)?;
        meta.staging_path = Some(staging.to_string_lossy().into_owned());

        let mut policy = sample_policy(temp.path());
        policy.extract = true;

        service.run_extract(torrent_id, &mut meta, &meta_path, &policy)?;

        assert_eq!(
            meta.step_status(StepKind::Extract),
            Some(StepStatus::Skipped)
        );
        Ok(())
    }

    #[test]
    fn run_verify_par2_skips_when_disabled() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let meta_path = temp.path().join("meta.json");
        let source = temp.path().join("source");
        fs::create_dir_all(&source)?;

        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        meta.source_path = Some(source.to_string_lossy().into_owned());

        let policy = sample_policy(temp.path());
        service.run_verify_par2(torrent_id, &mut meta, &meta_path, &policy)?;

        assert_eq!(
            meta.step_status(StepKind::VerifyPar2),
            Some(StepStatus::Skipped)
        );
        Ok(())
    }

    #[test]
    fn run_extract_disabled_preserves_source_as_staging() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");

        let source = temp.path().join("source");
        fs::create_dir_all(&source)?;
        meta.source_path = Some(source.to_string_lossy().into_owned());

        let policy = sample_policy(temp.path());
        service.run_extract(torrent_id, &mut meta, &meta_path, &policy)?;

        assert_eq!(
            meta.staging_path.as_deref(),
            Some(source.to_string_lossy().as_ref())
        );
        assert_eq!(
            meta.step_status(StepKind::Extract),
            Some(StepStatus::Skipped)
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn run_verify_par2_invokes_external_tool() -> TestResult<()> {
        let _lock = match external_tool_test_lock().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let temp = temp_dir()?;
        let log_path = temp.path().join("par2.log");
        let script = temp.path().join("par2");
        write_executable(
            &script,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\n",
                log_path.display()
            ),
        )?;
        set_tool_override("par2", &script);

        let source = temp.path().join("source");
        fs::create_dir_all(&source)?;
        fs::write(source.join("movie.mkv"), b"video")?;
        fs::write(source.join("movie.par2"), b"par2")?;

        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let meta_path = temp.path().join("meta.json");
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        meta.source_path = Some(source.to_string_lossy().into_owned());

        let mut policy = sample_policy(temp.path());
        policy.par2 = "repair".to_string();

        let result = service.run_verify_par2(torrent_id, &mut meta, &meta_path, &policy);

        clear_tool_overrides();

        result?;

        let log = fs::read_to_string(&log_path)?;
        assert!(log.contains('r'));
        assert!(log.contains("movie.par2"));
        assert_eq!(
            meta.step_status(StepKind::VerifyPar2),
            Some(StepStatus::Completed)
        );
        Ok(())
    }

    #[test]
    fn run_extract_requires_source_path_when_no_staging_exists() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");
        let work_dir = temp.path().join("work");
        fs::create_dir_all(&work_dir)?;
        meta.work_dir = Some(work_dir.to_string_lossy().into_owned());

        let mut policy = sample_policy(temp.path());
        policy.extract = true;

        let err = service
            .run_extract(torrent_id, &mut meta, &meta_path, &policy)
            .expect_err("expected missing source path to fail extraction");
        assert!(matches!(
            err,
            FsOpsError::MissingState {
                field: "source_path"
            }
        ));
        assert_eq!(
            meta.step_status(StepKind::Extract),
            Some(StepStatus::Failed)
        );
        Ok(())
    }

    #[test]
    fn run_flatten_skips_when_multiple_entries_exist() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");

        let staging = temp.path().join("staging");
        fs::create_dir_all(staging.join("one"))?;
        fs::create_dir_all(staging.join("two"))?;
        meta.staging_path = Some(staging.to_string_lossy().into_owned());

        let mut policy = sample_policy(temp.path());
        policy.flatten = true;

        service.run_flatten(torrent_id, &mut meta, &meta_path, &policy)?;

        assert_eq!(
            meta.step_status(StepKind::Flatten),
            Some(StepStatus::Skipped)
        );
        Ok(())
    }

    #[test]
    fn run_flatten_skips_when_staging_path_is_file() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");

        let staging = temp.path().join("archive.zip");
        fs::write(&staging, b"archive")?;
        meta.staging_path = Some(staging.to_string_lossy().into_owned());

        let mut policy = sample_policy(temp.path());
        policy.flatten = true;

        service.run_flatten(torrent_id, &mut meta, &meta_path, &policy)?;

        assert_eq!(
            meta.step_status(StepKind::Flatten),
            Some(StepStatus::Skipped)
        );
        Ok(())
    }

    #[test]
    fn transfer_step_rejects_unknown_move_mode() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");

        let root = temp.path().join("library");
        fs::create_dir_all(&root)?;
        let staging = temp.path().join("staging");
        fs::create_dir_all(&staging)?;
        meta.staging_path = Some(staging.to_string_lossy().into_owned());

        let mut policy = sample_policy(temp.path());
        policy.library_root = root.to_string_lossy().into_owned();
        policy.move_mode = "teleport".to_string();

        let err = service
            .run_transfer(torrent_id, &mut meta, &meta_path, &policy, &root)
            .err()
            .ok_or(FsOpsError::MissingState {
                field: "expected_transfer_error",
            })?;
        assert!(matches!(
            err,
            FsOpsError::InvalidPolicy {
                field: "move_mode",
                reason: "unsupported",
                ..
            }
        ));
        assert_eq!(
            meta.step_status(StepKind::Transfer),
            Some(StepStatus::Failed)
        );
        Ok(())
    }

    #[test]
    fn transfer_step_requires_staging_path() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");

        let root = temp.path().join("library");
        fs::create_dir_all(&root)?;
        let policy = FsPolicy {
            library_root: root.to_string_lossy().into_owned(),
            ..sample_policy(temp.path())
        };

        let err = service
            .run_transfer(torrent_id, &mut meta, &meta_path, &policy, &root)
            .expect_err("expected transfer without staging path to fail");
        assert!(matches!(
            err,
            FsOpsError::MissingState {
                field: "staging_path"
            }
        ));
        assert_eq!(
            meta.step_status(StepKind::Transfer),
            Some(StepStatus::Failed)
        );
        Ok(())
    }

    #[test]
    fn transfer_step_reports_parent_creation_failures() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");

        let root = temp.path().join("library");
        fs::create_dir_all(&root)?;
        let staging = temp.path().join("staging.txt");
        fs::write(&staging, b"payload")?;
        let blocked_parent = temp.path().join("blocked-parent");
        fs::write(&blocked_parent, b"not-a-directory")?;
        meta.staging_path = Some(staging.to_string_lossy().into_owned());
        meta.artifact_path = Some(
            blocked_parent
                .join("artifact.txt")
                .to_string_lossy()
                .into_owned(),
        );

        let policy = FsPolicy {
            library_root: root.to_string_lossy().into_owned(),
            ..sample_policy(temp.path())
        };

        let err = service
            .run_transfer(torrent_id, &mut meta, &meta_path, &policy, &root)
            .expect_err("expected blocked artifact parent to fail");
        assert!(matches!(
            err,
            FsOpsError::Io {
                operation: "transfer.create_parent",
                ..
            }
        ));
        assert_eq!(
            meta.step_status(StepKind::Transfer),
            Some(StepStatus::Failed)
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn hardlink_tree_reuses_inodes() -> TestResult<()> {
        let temp = temp_dir()?;
        let source = temp.path().join("source");
        fs::create_dir_all(&source)?;
        let file = source.join("file.txt");
        fs::write(&file, b"content")?;

        let destination = temp.path().join("dest");
        FsOpsService::hardlink_tree(&source, &destination)?;

        let dest_file = destination.join("file.txt");
        assert!(dest_file.exists());

        let src_meta = fs::metadata(&file)?;
        let dest_meta = fs::metadata(&dest_file)?;
        assert_eq!(src_meta.ino(), dest_meta.ino());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn hardlink_tree_links_file_source() -> TestResult<()> {
        let temp = temp_dir()?;
        let source = temp.path().join("source.txt");
        fs::write(&source, b"content")?;
        let destination = temp.path().join("dest.txt");

        FsOpsService::hardlink_tree(&source, &destination)?;

        let src_meta = fs::metadata(&source)?;
        let dest_meta = fs::metadata(&destination)?;
        assert_eq!(src_meta.ino(), dest_meta.ino());
        Ok(())
    }

    #[test]
    fn sanitize_archive_path_rejects_unsafe_inputs() -> TestResult<()> {
        assert!(
            FsOpsService::sanitize_archive_path("/abs/path").is_err(),
            "absolute entries should be rejected"
        );
        assert!(
            FsOpsService::sanitize_archive_path("../escape").is_err(),
            "parent traversal should be rejected"
        );
        let normalised = FsOpsService::sanitize_archive_path("nested/./file.txt")?;
        assert_eq!(normalised, PathBuf::from("nested/file.txt"));
        Ok(())
    }

    #[test]
    fn cleanup_destination_removes_matching_entries() -> TestResult<()> {
        let temp = temp_dir()?;
        let root = temp.path().join("artifact");
        fs::create_dir_all(root.join("keep"))?;
        fs::create_dir_all(root.join("extras"))?;
        fs::write(root.join("keep").join("movie.mkv"), b"video")?;
        fs::write(root.join("extras").join("note.nfo"), b"junk")?;

        let policy = FsPolicy {
            id: Uuid::new_v4(),
            library_root: root.display().to_string(),
            extract: false,
            par2: "disabled".to_string(),
            flatten: false,
            move_mode: "copy".to_string(),
            cleanup_keep: vec!["**/*.mkv".to_string()],
            cleanup_drop: vec!["**/*.nfo".to_string()],
            chmod_file: None,
            chmod_dir: None,
            owner: None,
            group: None,
            umask: None,
            allow_paths: Vec::new(),
        };
        let rules = RuleSet::from_policy(&policy)?;
        let removed = FsOpsService::cleanup_destination(&root, &rules);

        assert_eq!(removed, 1);
        assert!(root.join("keep").join("movie.mkv").exists());
        assert!(!root.join("extras").join("note.nfo").exists());
        Ok(())
    }

    #[test]
    fn checksum_step_records_file_and_manifest_hashes() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");

        let artifact = temp.path().join("artifact");
        fs::create_dir_all(&artifact)?;
        fs::write(artifact.join("episode1.mkv"), b"video")?;
        fs::write(artifact.join("episode1.srt"), b"subs")?;
        meta.artifact_path = Some(artifact.to_string_lossy().into_owned());

        service.run_checksum(torrent_id, &mut meta, &meta_path)?;

        assert_eq!(
            meta.step_status(StepKind::Checksum),
            Some(StepStatus::Completed)
        );
        assert_eq!(meta.checksums.len(), 3);
        assert!(
            meta.checksums
                .iter()
                .any(|record| matches!(record.kind, ChecksumKind::Manifest))
        );
        assert!(
            meta.checksums
                .iter()
                .filter(|record| matches!(record.kind, ChecksumKind::File))
                .all(|record| record.algorithm == "sha256")
        );
        let manifest = meta
            .checksums
            .iter()
            .find(|record| matches!(record.kind, ChecksumKind::Manifest))
            .ok_or(FsOpsError::MissingState {
                field: "manifest_checksum",
            })?;
        let manifest_bytes: u64 = meta
            .checksums
            .iter()
            .filter(|record| matches!(record.kind, ChecksumKind::File))
            .map(|record| {
                FsOpsService::checksum_manifest_line(&record.path, record.bytes, &record.value)
                    .len() as u64
            })
            .sum();
        assert_eq!(manifest.bytes, manifest_bytes);
        Ok(())
    }

    #[test]
    fn copy_tree_copies_file_source() -> TestResult<()> {
        let temp = temp_dir()?;
        let source = temp.path().join("source.txt");
        fs::write(&source, b"content")?;
        let destination = temp.path().join("dest.txt");

        FsOpsService::copy_tree(&source, &destination)?;

        assert!(destination.exists());
        assert_eq!(fs::read(&destination)?, b"content");
        Ok(())
    }

    #[test]
    fn move_tree_moves_file_source() -> TestResult<()> {
        let temp = temp_dir()?;
        let source = temp.path().join("source.txt");
        fs::write(&source, b"content")?;
        let destination = temp.path().join("dest.txt");

        FsOpsService::move_tree(&source, &destination)?;

        assert!(!source.exists());
        assert!(destination.exists());
        assert_eq!(fs::read(&destination)?, b"content");
        Ok(())
    }

    #[test]
    fn move_tree_falls_back_to_copy_when_rename_cannot_create_parent() -> TestResult<()> {
        let temp = temp_dir()?;
        let source = temp.path().join("source.txt");
        fs::write(&source, b"content")?;
        let destination = temp.path().join("nested").join("dest.txt");

        FsOpsService::move_tree(&source, &destination)?;

        assert!(!source.exists());
        assert_eq!(fs::read(&destination)?, b"content");
        Ok(())
    }

    #[test]
    fn copy_tree_copies_directory_source() -> TestResult<()> {
        let temp = temp_dir()?;
        let source = temp.path().join("source");
        let nested = source.join("nested");
        fs::create_dir_all(&nested)?;
        fs::write(nested.join("episode.mkv"), b"content")?;

        let destination = temp.path().join("dest");
        FsOpsService::copy_tree(&source, &destination)?;

        assert!(destination.join("nested").is_dir());
        assert_eq!(
            fs::read(destination.join("nested").join("episode.mkv"))?,
            b"content"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn extract_zip_creates_directories_and_preserves_permissions() -> TestResult<()> {
        use std::os::unix::fs::PermissionsExt;

        let temp = temp_dir()?;
        let archive = temp.path().join("payload.zip");
        let file = File::create(&archive)?;
        let mut zip = zip::ZipWriter::new(file);
        let dir_options = zip::write::FileOptions::default().unix_permissions(0o755);
        let file_options = zip::write::FileOptions::default().unix_permissions(0o744);
        zip.add_directory("show/Season1/", dir_options)?;
        zip.start_file("show/Season1/episode.mkv", file_options)?;
        zip.write_all(b"video")?;
        zip.finish()?;

        let target = temp.path().join("target");
        FsOpsService::extract_zip(&archive, &target)?;

        let season_dir = target.join("show").join("Season1");
        let episode = season_dir.join("episode.mkv");
        assert!(season_dir.is_dir());
        assert!(episode.exists());
        assert_eq!(
            fs::metadata(&season_dir)?.permissions().mode() & 0o777,
            0o755
        );
        assert_eq!(fs::metadata(&episode)?.permissions().mode() & 0o777, 0o744);
        Ok(())
    }

    #[test]
    fn fsops_meta_updates_status_and_timestamps() {
        let torrent_id = Uuid::new_v4();
        let policy_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, policy_id);
        let first_updated = meta.updated_at;

        assert!(
            meta.update_step(StepKind::Cleanup, StepStatus::Started, Some("begin".into())),
            "first update should record new step"
        );
        let second_updated = meta.updated_at;
        assert!(second_updated >= first_updated);
        assert!(
            !meta.update_step(StepKind::Cleanup, StepStatus::Started, Some("begin".into())),
            "repeating identical update should be a no-op"
        );
        assert!(
            meta.update_step(
                StepKind::Cleanup,
                StepStatus::Completed,
                Some("done".into())
            ),
            "changed status should be persisted"
        );
        assert_eq!(
            meta.step_status(StepKind::Cleanup),
            Some(StepStatus::Completed)
        );
        assert!(meta.updated_at >= second_updated);
    }

    #[test]
    fn parse_octal_mode_validates_values() -> TestResult<()> {
        assert_eq!(
            FsOpsService::parse_octal_mode("chmod_file", "0o755")?,
            0o755
        );
        assert!(FsOpsService::parse_octal_mode("chmod_file", "not-a-mode").is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn apply_permissions_reports_unchanged_without_directives() -> TestResult<()> {
        use std::os::unix::fs::PermissionsExt;

        let temp = temp_dir()?;
        let root = temp.path().join("artifact");
        fs::create_dir_all(&root)?;
        let file_path = root.join("file.txt");
        fs::write(&file_path, b"content")?;

        let before = fs::metadata(&file_path)?.permissions().mode() & 0o777;

        let detail = FsOpsService::apply_permissions(&root, None, None, None, None, None)?;
        assert_eq!(detail, "permissions=unchanged");

        let after = fs::metadata(&file_path)?.permissions().mode() & 0o777;
        assert_eq!(before, after);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn apply_permissions_applies_explicit_modes() -> TestResult<()> {
        use std::os::unix::fs::PermissionsExt;

        let temp = temp_dir()?;
        let root = temp.path().join("artifact");
        fs::create_dir_all(&root)?;
        let nested = root.join("dir");
        fs::create_dir_all(&nested)?;
        let file_path = nested.join("file.txt");
        fs::write(&file_path, b"content")?;

        let detail =
            FsOpsService::apply_permissions(&root, Some("0o600"), Some("0o700"), None, None, None)?;
        assert!(
            detail.contains("file=0o600") && detail.contains("dir=0o700"),
            "expected explicit modes to be reported"
        );

        let file_mode = fs::metadata(&file_path)?.permissions().mode() & 0o777;
        let dir_mode = fs::metadata(&nested)?.permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600);
        assert_eq!(dir_mode, 0o700);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn resolve_owner_and_group_accept_numeric_ids() -> TestResult<()> {
        let uid = Uid::current().as_raw();
        let gid = Gid::current().as_raw();

        let (owner_id, owner_label) = FsOpsService::resolve_owner(&uid.to_string())?;
        assert_eq!(owner_id.as_raw(), uid);
        assert!(owner_label.contains("uid("));

        let (group_id, group_label) = FsOpsService::resolve_group(&gid.to_string())?;
        assert_eq!(group_id.as_raw(), gid);
        assert!(group_label.contains("gid("));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn resolve_owner_and_group_reject_empty_values() {
        assert!(matches!(
            FsOpsService::resolve_owner("  ").expect_err("expected empty owner error"),
            FsOpsError::InvalidInput {
                field: "owner",
                reason: "empty",
                ..
            }
        ));
        assert!(matches!(
            FsOpsService::resolve_group("  ").expect_err("expected empty group error"),
            FsOpsError::InvalidInput {
                field: "group",
                reason: "empty",
                ..
            }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn resolve_owner_and_group_reject_unknown_values() {
        let owner_value = format!("owner-{}", Uuid::new_v4());
        let group_value = format!("group-{}", Uuid::new_v4());

        assert!(matches!(
            FsOpsService::resolve_owner(&owner_value).expect_err("expected missing owner"),
            FsOpsError::InvalidInput {
                field: "owner",
                reason: "not_found",
                ..
            }
        ));
        assert!(matches!(
            FsOpsService::resolve_group(&group_value).expect_err("expected missing group"),
            FsOpsError::InvalidInput {
                field: "group",
                reason: "not_found",
                ..
            }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn apply_permissions_honours_umask_defaults() -> TestResult<()> {
        let temp = temp_dir()?;
        let root = temp.path().join("artifact");
        fs::create_dir_all(&root)?;
        let nested = root.join("dir");
        fs::create_dir_all(&nested)?;
        let file_path = nested.join("file.txt");
        fs::write(&file_path, b"content")?;

        let detail = FsOpsService::apply_permissions(&root, None, None, None, None, Some("0o022"))?;
        assert!(
            detail.contains("file=0o644") && detail.contains("dir=0o755"),
            "expected derived permissions to be applied"
        );

        let file_mode = fs::metadata(&file_path)?.permissions().mode() & 0o777;
        let dir_mode = fs::metadata(&nested)?.permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o644);
        assert_eq!(dir_mode, 0o755);
        Ok(())
    }

    #[test]
    fn cleanup_step_skips_without_artifact_path() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");
        let policy = sample_policy(temp.path());

        service.run_cleanup(torrent_id, &mut meta, &meta_path, &policy)?;

        assert_eq!(
            meta.step_status(StepKind::Cleanup),
            Some(StepStatus::Skipped)
        );
        Ok(())
    }

    #[test]
    fn set_permissions_step_skips_without_artifact_path() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let mut meta = FsOpsMeta::new(Uuid::new_v4(), Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");
        let policy = sample_policy(temp.path());

        service.run_set_permissions(meta.torrent_id, &mut meta, &meta_path, &policy)?;

        assert_eq!(
            meta.step_status(StepKind::SetPermissions),
            Some(StepStatus::Skipped)
        );
        Ok(())
    }

    #[test]
    fn set_permissions_step_skips_when_artifact_missing_on_disk() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        meta.artifact_path = Some(temp.path().join("missing").to_string_lossy().into_owned());
        let meta_path = temp.path().join("meta.json");
        let mut policy = sample_policy(temp.path());
        policy.chmod_file = Some("0o644".to_string());

        service.run_set_permissions(torrent_id, &mut meta, &meta_path, &policy)?;

        assert_eq!(
            meta.step_status(StepKind::SetPermissions),
            Some(StepStatus::Skipped)
        );
        Ok(())
    }

    #[test]
    fn set_permissions_step_applies_modes_and_ownership() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let artifact = temp.path().join("artifact");
        let nested = artifact.join("dir");
        fs::create_dir_all(&nested)?;
        let file_path = nested.join("file.txt");
        fs::write(&file_path, b"content")?;
        meta.artifact_path = Some(artifact.to_string_lossy().into_owned());
        let meta_path = temp.path().join("meta.json");
        let current = fs::metadata(&artifact)?;
        let mut policy = sample_policy(temp.path());
        policy.chmod_file = Some("0o640".to_string());
        policy.chmod_dir = Some("0o750".to_string());
        policy.owner = Some(current.uid().to_string());
        policy.group = Some(current.gid().to_string());

        service.run_set_permissions(torrent_id, &mut meta, &meta_path, &policy)?;

        assert_eq!(
            meta.step_status(StepKind::SetPermissions),
            Some(StepStatus::Completed)
        );
        let persisted = load_meta(&meta_path)?;
        let detail = persisted
            .steps
            .iter()
            .find(|record| record.name == StepKind::SetPermissions.as_str())
            .and_then(|record| record.detail.clone())
            .ok_or_else(|| anyhow::anyhow!("missing permission detail"))?;
        assert!(detail.contains("file=0o640"));
        assert!(detail.contains("dir=0o750"));
        assert!(detail.contains("owner=uid("));
        assert!(detail.contains("group=gid("));
        Ok(())
    }

    #[test]
    fn cleanup_step_skips_when_no_rules_configured() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");

        let artifact = temp.path().join("artifact_dir");
        fs::create_dir_all(&artifact)?;
        meta.artifact_path = Some(artifact.to_string_lossy().into_owned());

        let mut policy = sample_policy(temp.path());
        policy.cleanup_keep = Vec::new();
        policy.cleanup_drop = Vec::new();

        service.run_cleanup(torrent_id, &mut meta, &meta_path, &policy)?;

        assert_eq!(
            meta.step_status(StepKind::Cleanup),
            Some(StepStatus::Skipped)
        );
        Ok(())
    }

    #[test]
    fn cleanup_step_skips_when_artifact_is_file() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let artifact = temp.path().join("artifact.txt");
        fs::write(&artifact, b"content")?;
        meta.artifact_path = Some(artifact.to_string_lossy().into_owned());
        let meta_path = temp.path().join("meta.json");
        let mut policy = sample_policy(temp.path());
        policy.cleanup_drop = vec!["**/*.txt".to_string()];

        service.run_cleanup(torrent_id, &mut meta, &meta_path, &policy)?;

        assert_eq!(
            meta.step_status(StepKind::Cleanup),
            Some(StepStatus::Skipped)
        );
        Ok(())
    }

    #[test]
    fn transfer_step_skips_when_destination_already_positioned() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");

        let root = temp.path().join("library");
        let staged = root.join("title");
        fs::create_dir_all(&staged)?;
        meta.staging_path = Some(staged.to_string_lossy().into_owned());

        let policy = FsPolicy {
            library_root: root.to_string_lossy().into_owned(),
            ..sample_policy(temp.path())
        };

        service.run_transfer(torrent_id, &mut meta, &meta_path, &policy, &root)?;

        assert_eq!(
            meta.step_status(StepKind::Transfer),
            Some(StepStatus::Skipped)
        );
        assert_eq!(meta.transfer_mode.as_deref(), Some("copy"));
        Ok(())
    }

    #[test]
    fn transfer_step_replaces_existing_file_destination() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");

        let root = temp.path().join("library");
        fs::create_dir_all(&root)?;
        let staging = temp.path().join("title.txt");
        fs::write(&staging, b"new")?;
        let destination = root.join("title.txt");
        fs::write(&destination, b"old")?;
        meta.staging_path = Some(staging.to_string_lossy().into_owned());

        let policy = FsPolicy {
            library_root: root.to_string_lossy().into_owned(),
            ..sample_policy(temp.path())
        };

        service.run_transfer(torrent_id, &mut meta, &meta_path, &policy, &root)?;

        assert_eq!(fs::read(&destination)?, b"new");
        assert_eq!(
            meta.step_status(StepKind::Transfer),
            Some(StepStatus::Completed)
        );
        Ok(())
    }

    #[test]
    fn transfer_step_replaces_existing_directory_destination() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");

        let root = temp.path().join("library");
        fs::create_dir_all(&root)?;
        let staging = temp.path().join("show");
        fs::create_dir_all(&staging)?;
        fs::write(staging.join("episode.mkv"), b"new")?;

        let destination = root.join("show");
        fs::create_dir_all(&destination)?;
        fs::write(destination.join("stale.txt"), b"old")?;
        meta.staging_path = Some(staging.to_string_lossy().into_owned());

        let policy = FsPolicy {
            library_root: root.to_string_lossy().into_owned(),
            ..sample_policy(temp.path())
        };

        service.run_transfer(torrent_id, &mut meta, &meta_path, &policy, &root)?;

        assert!(!destination.join("stale.txt").exists());
        assert_eq!(fs::read(destination.join("episode.mkv"))?, b"new");
        assert_eq!(
            meta.step_status(StepKind::Transfer),
            Some(StepStatus::Completed)
        );
        Ok(())
    }

    #[test]
    fn transfer_step_supports_hardlink_mode_with_inferred_destination() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let meta_path = temp.path().join("meta.json");

        let root = temp.path().join("library");
        fs::create_dir_all(&root)?;
        let staging = temp.path().join("show");
        fs::create_dir_all(&staging)?;
        let staged_file = staging.join("episode.mkv");
        fs::write(&staged_file, b"linked")?;
        meta.staging_path = Some(staging.to_string_lossy().into_owned());

        let policy = FsPolicy {
            library_root: root.to_string_lossy().into_owned(),
            move_mode: "hardlink".to_string(),
            ..sample_policy(temp.path())
        };

        service.run_transfer(torrent_id, &mut meta, &meta_path, &policy, &root)?;

        let destination = root.join("show");
        let destination_file = destination.join("episode.mkv");
        assert_eq!(fs::read(&destination_file)?, b"linked");
        assert_eq!(
            fs::metadata(&staged_file)?.ino(),
            fs::metadata(&destination_file)?.ino()
        );
        assert_eq!(meta.transfer_mode.as_deref(), Some("hardlink"));
        assert_eq!(
            meta.artifact_path.as_deref(),
            Some(destination.to_string_lossy().as_ref())
        );
        assert_eq!(
            meta.step_status(StepKind::Transfer),
            Some(StepStatus::Completed)
        );
        Ok(())
    }

    #[tokio::test]
    async fn apply_with_runtime_records_started_and_completed_job_state() -> TestResult<()> {
        let Some((postgres, store)) = runtime_store().await? else {
            return Ok(());
        };
        let _keep_db_alive = postgres;

        let bus = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus.clone(), metrics).with_runtime(store.clone());
        let torrent_id = Uuid::new_v4();
        seed_runtime_torrent(&store, torrent_id).await?;

        let temp = temp_dir()?;
        let source = temp.path().join("source");
        fs::create_dir_all(&source)?;
        fs::write(source.join("episode.mkv"), b"video")?;
        let policy = sample_policy(temp.path());

        service.apply(FsOpsRequest {
            torrent_id,
            source_path: &source,
            policy: &policy,
        })?;

        let expected_destination = Path::new(&policy.library_root)
            .join("source")
            .to_string_lossy()
            .into_owned();
        let (status, attempt, src_path, dst_path, transfer_mode, last_error) =
            wait_for_fs_job_state(&store, torrent_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("missing persisted fs job state"))?;

        assert_eq!(status, "moved");
        assert_eq!(attempt, 1);
        assert_eq!(src_path, source.to_string_lossy());
        assert_eq!(dst_path.as_deref(), Some(expected_destination.as_str()));
        assert_eq!(transfer_mode.as_deref(), Some("copy"));
        assert_eq!(last_error, None);
        assert!(bus.backlog_since(0).into_iter().any(|envelope| {
            matches!(
                envelope.event,
                Event::FsopsCompleted { torrent_id: id } if id == torrent_id
            )
        }));
        Ok(())
    }

    #[tokio::test]
    async fn apply_with_runtime_records_failed_job_state() -> TestResult<()> {
        let Some((postgres, store)) = runtime_store().await? else {
            return Ok(());
        };
        let _keep_db_alive = postgres;

        let bus = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus.clone(), metrics).with_runtime(store.clone());
        let torrent_id = Uuid::new_v4();
        seed_runtime_torrent(&store, torrent_id).await?;

        let temp = temp_dir()?;
        let missing = temp.path().join("missing-source");
        let policy = sample_policy(temp.path());

        let err = service
            .apply(FsOpsRequest {
                torrent_id,
                source_path: &missing,
                policy: &policy,
            })
            .expect_err("expected missing source path to fail");
        assert!(matches!(
            err,
            FsOpsError::InvalidInput {
                field: "source_path",
                reason: "missing",
                ..
            }
        ));

        let (status, attempt, src_path, dst_path, transfer_mode, last_error) =
            wait_for_fs_job_state(&store, torrent_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("missing failed fs job state"))?;

        assert_eq!(status, "failed");
        assert!(attempt >= 2);
        assert_eq!(src_path, missing.to_string_lossy());
        assert_eq!(dst_path, None);
        assert_eq!(transfer_mode, None);
        let detail = last_error.ok_or_else(|| anyhow::anyhow!("missing failure detail"))?;
        assert!(!detail.trim().is_empty());
        assert!(bus.backlog_since(0).into_iter().any(|envelope| {
            matches!(
                envelope.event,
                Event::FsopsFailed { torrent_id: id, .. } if id == torrent_id
            )
        }));
        Ok(())
    }

    #[tokio::test]
    async fn apply_succeeds_when_runtime_pool_is_closed() -> TestResult<()> {
        let Some((postgres, store)) = runtime_store().await? else {
            return Ok(());
        };
        let _keep_db_alive = postgres;

        let bus = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus.clone(), metrics).with_runtime(store.clone());
        let torrent_id = Uuid::new_v4();
        let temp = temp_dir()?;
        let source = temp.path().join("source");
        fs::create_dir_all(&source)?;
        fs::write(source.join("episode.mkv"), b"video")?;
        let policy = sample_policy(temp.path());

        store.pool().close().await;
        service.apply(FsOpsRequest {
            torrent_id,
            source_path: &source,
            policy: &policy,
        })?;
        sleep(Duration::from_millis(100)).await;

        assert!(bus.backlog_since(0).into_iter().any(|envelope| {
            matches!(
                envelope.event,
                Event::FsopsCompleted { torrent_id: id } if id == torrent_id
            )
        }));
        Ok(())
    }

    #[tokio::test]
    async fn apply_failure_is_preserved_when_runtime_pool_is_closed() -> TestResult<()> {
        let Some((postgres, store)) = runtime_store().await? else {
            return Ok(());
        };
        let _keep_db_alive = postgres;

        let bus = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus.clone(), metrics).with_runtime(store.clone());
        let torrent_id = Uuid::new_v4();
        let temp = temp_dir()?;
        let missing = temp.path().join("missing-source");
        let policy = sample_policy(temp.path());

        store.pool().close().await;
        let err = service
            .apply(FsOpsRequest {
                torrent_id,
                source_path: &missing,
                policy: &policy,
            })
            .expect_err("expected missing source path to fail even without runtime persistence");
        assert!(matches!(
            err,
            FsOpsError::InvalidInput {
                field: "source_path",
                reason: "missing",
                ..
            }
        ));
        sleep(Duration::from_millis(100)).await;

        assert!(bus.backlog_since(0).into_iter().any(|envelope| {
            matches!(
                envelope.event,
                Event::FsopsFailed { torrent_id: id, .. } if id == torrent_id
            )
        }));
        Ok(())
    }

    #[tokio::test]
    async fn record_job_completed_marks_job_failed_when_source_is_missing() -> TestResult<()> {
        let Some((postgres, store)) = runtime_store().await? else {
            return Ok(());
        };
        let _keep_db_alive = postgres;

        let metrics = Metrics::new()?;
        let service =
            FsOpsService::new(EventBus::with_capacity(4), metrics).with_runtime(store.clone());
        let torrent_id = Uuid::new_v4();
        seed_runtime_torrent(&store, torrent_id).await?;
        store
            .mark_fs_job_started(torrent_id, Path::new(".server_root/source"))
            .await?;

        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        meta.artifact_path = Some(".server_root/library/artifact".to_string());

        service.record_job_completed(torrent_id, &meta);

        let (status, attempt, _src_path, _dst_path, _transfer_mode, last_error) =
            wait_for_fs_job_state(&store, torrent_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("missing fallback fs job state"))?;
        assert_eq!(status, "failed");
        assert_eq!(attempt, 2);
        assert_eq!(
            last_error.as_deref(),
            Some("fsops completed without recorded source path")
        );
        Ok(())
    }

    #[tokio::test]
    async fn record_job_completed_marks_job_failed_when_artifact_is_missing() -> TestResult<()> {
        let Some((postgres, store)) = runtime_store().await? else {
            return Ok(());
        };
        let _keep_db_alive = postgres;

        let metrics = Metrics::new()?;
        let service =
            FsOpsService::new(EventBus::with_capacity(4), metrics).with_runtime(store.clone());
        let torrent_id = Uuid::new_v4();
        seed_runtime_torrent(&store, torrent_id).await?;
        store
            .mark_fs_job_started(torrent_id, Path::new(".server_root/source"))
            .await?;

        let meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        service.record_job_completed(torrent_id, &meta);

        let (status, attempt, _src_path, _dst_path, _transfer_mode, last_error) =
            wait_for_fs_job_state(&store, torrent_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("missing artifact fallback fs job state"))?;
        assert_eq!(status, "failed");
        assert_eq!(attempt, 2);
        assert_eq!(
            last_error.as_deref(),
            Some("fsops completed without artifact")
        );
        Ok(())
    }

    #[test]
    fn apply_emits_single_degraded_transition_then_recovers() -> TestResult<()> {
        let bus = EventBus::with_capacity(32);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus.clone(), metrics);
        let temp = temp_dir()?;
        let policy = sample_policy(temp.path());
        let missing = temp.path().join("missing");

        assert!(
            service
                .apply(FsOpsRequest {
                    torrent_id: Uuid::new_v4(),
                    source_path: &missing,
                    policy: &policy,
                })
                .is_err()
        );

        assert!(
            service
                .apply(FsOpsRequest {
                    torrent_id: Uuid::new_v4(),
                    source_path: &missing,
                    policy: &policy,
                })
                .is_err()
        );

        let degraded_events = bus
            .backlog_since(0)
            .into_iter()
            .filter(|envelope| {
                matches!(
                    &envelope.event,
                    Event::HealthChanged { degraded } if degraded == &vec![HEALTH_COMPONENT.to_string()]
                )
            })
            .count();
        assert_eq!(degraded_events, 1);

        let source = temp.path().join("source");
        fs::create_dir_all(&source)?;
        fs::write(source.join("episode.mkv"), b"video")?;
        service.apply(FsOpsRequest {
            torrent_id: Uuid::new_v4(),
            source_path: &source,
            policy: &policy,
        })?;

        assert!(bus.backlog_since(0).into_iter().any(|envelope| {
            matches!(
                envelope.event,
                Event::HealthChanged { degraded } if degraded.is_empty()
            )
        }));
        Ok(())
    }

    #[test]
    fn health_transitions_emit_single_degradation_and_recovery() -> TestResult<()> {
        let bus = EventBus::with_capacity(16);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus.clone(), metrics);

        service.mark_degraded("first failure");
        service.mark_degraded("second failure");
        service.mark_recovered();

        let health_events = bus
            .backlog_since(0)
            .into_iter()
            .filter_map(|envelope| match envelope.event {
                Event::HealthChanged { degraded } => Some(degraded),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            health_events,
            vec![vec![HEALTH_COMPONENT.to_string()], Vec::new()]
        );
        Ok(())
    }

    #[test]
    fn finalise_step_removes_work_dir_and_uses_default_detail() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        let work_dir = temp.path().join("work");
        fs::create_dir_all(&work_dir)?;
        meta.work_dir = Some(work_dir.to_string_lossy().into_owned());
        let meta_path = temp.path().join("meta.json");

        service.run_finalise(torrent_id, &mut meta, &meta_path)?;

        assert!(!work_dir.exists());
        assert!(meta.completed);
        let persisted = load_meta(&meta_path)?;
        let detail = persisted
            .steps
            .iter()
            .find(|record| record.name == StepKind::Finalise.as_str())
            .and_then(|record| record.detail.as_deref())
            .ok_or_else(|| anyhow::anyhow!("missing finalise detail"))?;
        assert_eq!(detail, "artifact=<unset>");
        Ok(())
    }

    #[test]
    fn resume_short_circuits_completed_pipeline() -> TestResult<()> {
        let temp = temp_dir()?;
        let bus = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus.clone(), metrics);
        let torrent_id = Uuid::new_v4();
        let root = temp.path().join("library");
        fs::create_dir_all(&root)?;

        let meta_dir = root.join(META_DIR_NAME);
        fs::create_dir_all(&meta_dir)?;
        let meta_path = meta_dir.join(format!("{torrent_id}{META_SUFFIX}"));
        let artifact = root.join("artifact");
        fs::create_dir_all(&artifact)?;

        let mut meta = FsOpsMeta::new(torrent_id, Uuid::new_v4());
        meta.completed = true;
        meta.artifact_path = Some(artifact.to_string_lossy().into_owned());
        meta.update_step(StepKind::Finalise, StepStatus::Completed, None);
        persist_meta(&meta_path, &meta)?;

        let policy = sample_policy(temp.path());
        let mut stream = bus.subscribe(None);
        service.apply(FsOpsRequest {
            torrent_id,
            source_path: &artifact,
            policy: &policy,
        })?;

        let persisted = load_meta(&meta_path)?;
        assert!(
            persisted.completed,
            "resume should preserve completion flag"
        );

        let runtime = Runtime::new()?;
        let completed = runtime.block_on(async {
            while let Some(result) = stream.next().await {
                let envelope = match result {
                    Ok(envelope) => envelope,
                    Err(err) => {
                        return Err(FsOpsError::InvalidInput {
                            field: "event_stream",
                            reason: "recv_error",
                            value: Some(err.to_string()),
                        });
                    }
                };
                if matches!(
                    envelope.event,
                    Event::FsopsCompleted { torrent_id: id } if id == torrent_id
                ) {
                    return Ok(true);
                }
            }
            Ok(false)
        })?;
        assert!(completed, "expected completion event for resumed job");
        Ok(())
    }
}
