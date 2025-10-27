//! Filesystem post-processing pipeline for completed torrents.
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
#![allow(clippy::module_name_repetitions, clippy::multiple_crate_versions)]

use std::{
    fs::{self, File},
    io,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use chrono::{DateTime, Utc};
use globset::{Glob, GlobSet, GlobSetBuilder};
use revaer_config::FsPolicy;
use revaer_events::{Event, EventBus};
use revaer_telemetry::Metrics;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, warn};
use uuid::Uuid;
use walkdir::WalkDir;
use zip::ZipArchive;

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
    PrepareWorkDir,
    Extract,
    Flatten,
    Transfer,
    SetPermissions,
    Cleanup,
    Finalise,
}

impl StepKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ValidatePolicy => "validate_policy",
            Self::Allowlist => "allowlist",
            Self::PrepareDirectories => "prepare_directories",
            Self::CompileRules => "compile_rules",
            Self::LocateSource => "locate_source",
            Self::PrepareWorkDir => "prepare_work_dir",
            Self::Extract => "extract",
            Self::Flatten => "flatten",
            Self::Transfer => "transfer",
            Self::SetPermissions => "set_permissions",
            Self::Cleanup => "cleanup",
            Self::Finalise => "finalise",
        }
    }

    const fn progress_label(self) -> &'static str {
        match self {
            Self::ValidatePolicy => "validate",
            Self::Allowlist => "allowlist",
            Self::PrepareDirectories => "prepare_directories",
            Self::CompileRules => "compile_rules",
            Self::LocateSource => "locate_source",
            Self::PrepareWorkDir => "prepare_work_dir",
            Self::Extract => "extract",
            Self::Flatten => "flatten",
            Self::Transfer => "transfer",
            Self::SetPermissions => "set_permissions",
            Self::Cleanup => "cleanup",
            Self::Finalise => "finalise",
        }
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

/// Immutable inputs provided to the filesystem pipeline for a completed torrent.
#[derive(Copy, Clone)]
pub struct FsOpsRequest<'a> {
    /// Identifier of the torrent the operation applies to.
    pub torrent_id: Uuid,
    /// Absolute staging path that contains the downloaded payload.
    pub source_path: &'a Path,
    /// Filesystem policy snapshot describing how to handle the payload.
    pub policy: &'a FsPolicy,
}

/// Service responsible for executing filesystem post-processing steps after torrent completion.
#[derive(Clone)]
pub struct FsOpsService {
    events: EventBus,
    metrics: Metrics,
    health_degraded: Arc<Mutex<bool>>,
}

impl FsOpsService {
    /// Construct a new filesystem operations service backed by the shared event bus.
    #[must_use]
    pub fn new(events: EventBus, metrics: Metrics) -> Self {
        Self {
            events,
            metrics,
            health_degraded: Arc::new(Mutex::new(false)),
        }
    }

    /// Apply the configured filesystem policy for the given torrent and emit progress events.
    ///
    /// # Errors
    ///
    /// Returns an error if any filesystem post-processing step fails.
    pub fn apply(&self, request: FsOpsRequest<'_>) -> Result<()> {
        let _ = self.events.publish(Event::FsopsStarted {
            torrent_id: request.torrent_id,
        });

        let result = self.execute_pipeline(&request);

        match &result {
            Ok(()) => {
                self.mark_recovered();
                let _ = self.events.publish(Event::FsopsCompleted {
                    torrent_id: request.torrent_id,
                });
            }
            Err(error) => {
                self.mark_degraded(&format!("{error:#}"));
                let _ = self.events.publish(Event::FsopsFailed {
                    torrent_id: request.torrent_id,
                    message: format!("{error:#}"),
                });
            }
        }

        result
    }

    #[allow(clippy::too_many_lines)]
    fn execute_pipeline(&self, request: &FsOpsRequest<'_>) -> Result<()> {
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
            return Ok(());
        }

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::ValidatePolicy,
            StepPersistence::new(false, false, false),
            {
                let root_value = policy.library_root.clone();
                move |_meta| {
                    ensure!(
                        !root_value.trim().is_empty(),
                        "filesystem policy library root cannot be empty"
                    );
                    Ok(StepOutcome::Completed(Some(format!(
                        "library_root={root_value}"
                    ))))
                }
            },
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::Allowlist,
            StepPersistence::new(false, false, false),
            {
                let allow_paths = policy.allow_paths.clone();
                let root_clone = root.clone();
                move |_meta| {
                    enforce_allow_paths(&root_clone, &allow_paths)?;
                    Ok(StepOutcome::Completed(None))
                }
            },
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::PrepareDirectories,
            StepPersistence::new(false, true, false),
            {
                let root_clone = root.clone();
                let meta_dir_clone = meta_dir.clone();
                move |_meta| {
                    fs::create_dir_all(&root_clone).with_context(|| {
                        format!(
                            "failed to create library root directory at {}",
                            root_clone.display()
                        )
                    })?;
                    fs::create_dir_all(&meta_dir_clone).with_context(|| {
                        format!(
                            "failed to create fsops metadata directory at {}",
                            meta_dir_clone.display()
                        )
                    })?;
                    Ok(StepOutcome::Completed(Some(format!(
                        "meta_dir={}",
                        meta_dir_clone.display()
                    ))))
                }
            },
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
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
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::LocateSource,
            StepPersistence::new(false, true, false),
            {
                let explicit_source = source_path.to_path_buf();
                move |meta| {
                    let candidate = meta
                        .source_path
                        .as_ref()
                        .map_or_else(|| explicit_source.clone(), PathBuf::from);
                    let canonical = candidate
                        .canonicalize()
                        .unwrap_or_else(|_| candidate.clone());
                    ensure!(
                        canonical.exists(),
                        "fsops source path {} does not exist",
                        canonical.display()
                    );
                    let encoded = canonical.to_string_lossy().into_owned();
                    meta.source_path = Some(encoded);
                    if meta.staging_path.is_none() {
                        meta.staging_path = meta.source_path.clone();
                    }
                    Ok(StepOutcome::Completed(Some(format!(
                        "source={}",
                        canonical.display()
                    ))))
                }
            },
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::PrepareWorkDir,
            StepPersistence::new(false, true, false),
            {
                let default_work_dir = meta_dir.join("work").join(torrent_id.to_string());
                move |meta| {
                    let work_dir_path = meta
                        .work_dir
                        .as_ref()
                        .map_or_else(|| default_work_dir.clone(), PathBuf::from);
                    fs::create_dir_all(&work_dir_path).with_context(|| {
                        format!(
                            "failed to prepare fsops work directory at {}",
                            work_dir_path.display()
                        )
                    })?;
                    meta.work_dir = Some(work_dir_path.to_string_lossy().into_owned());
                    Ok(StepOutcome::Completed(Some(format!(
                        "work_dir={}",
                        work_dir_path.display()
                    ))))
                }
            },
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::Extract,
            StepPersistence::new(false, true, false),
            {
                let extract_enabled = policy.extract;
                move |meta| {
                    if !extract_enabled {
                        if meta.staging_path.is_none() {
                            meta.staging_path = meta.source_path.clone();
                        }
                        return Ok(StepOutcome::Skipped(Some("extract disabled".into())));
                    }
                    let staging = meta.staging_path.as_ref().map_or_else(
                        || {
                            PathBuf::from(
                                meta.source_path
                                    .as_ref()
                                    .expect("source path initialised before extraction"),
                            )
                        },
                        PathBuf::from,
                    );
                    if staging.is_dir() {
                        return Ok(StepOutcome::Skipped(Some(
                            "source already directory; nothing to extract".into(),
                        )));
                    }
                    let work_dir = meta
                        .work_dir
                        .as_ref()
                        .map(PathBuf::from)
                        .ok_or_else(|| anyhow!("fsops work directory not prepared"))?;
                    let extraction_target = work_dir.join("extracted");
                    if extraction_target.exists() {
                        fs::remove_dir_all(&extraction_target).with_context(|| {
                            format!(
                                "failed to reset extraction directory {}",
                                extraction_target.display()
                            )
                        })?;
                    }
                    fs::create_dir_all(&extraction_target).with_context(|| {
                        format!(
                            "failed to create extraction directory {}",
                            extraction_target.display()
                        )
                    })?;
                    Self::extract_archive(&staging, &extraction_target)?;
                    meta.staging_path = Some(extraction_target.to_string_lossy().into_owned());
                    Ok(StepOutcome::Completed(Some(format!(
                        "archive={}",
                        staging.display()
                    ))))
                }
            },
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::Flatten,
            StepPersistence::new(false, true, false),
            {
                let flatten_enabled = policy.flatten;
                move |meta| {
                    if !flatten_enabled {
                        return Ok(StepOutcome::Skipped(Some("flatten disabled".into())));
                    }
                    let staging = meta
                        .staging_path
                        .as_ref()
                        .map(PathBuf::from)
                        .ok_or_else(|| anyhow!("staging path unavailable before flatten step"))?;
                    if !staging.is_dir() {
                        return Ok(StepOutcome::Skipped(Some(
                            "staging path is not a directory".into(),
                        )));
                    }
                    let mut entries = fs::read_dir(&staging)
                        .with_context(|| {
                            format!(
                                "failed to enumerate staging directory {}",
                                staging.display()
                            )
                        })?
                        .filter_map(Result::ok)
                        .collect::<Vec<_>>();
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
                }
            },
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::Transfer,
            StepPersistence::new(false, true, false),
            {
                let move_mode = policy.move_mode.as_str();
                let root_ref = &root;
                let torrent_label = torrent_id.to_string();
                move |meta| {
                    let staging = meta
                        .staging_path
                        .as_ref()
                        .map(PathBuf::from)
                        .ok_or_else(|| anyhow!("staging path unavailable before transfer"))?;
                    let destination = meta.artifact_path.as_ref().map_or_else(
                        || {
                            let inferred = staging
                                .file_name()
                                .and_then(|name| name.to_str())
                                .filter(|name| !name.is_empty())
                                .map_or_else(
                                    || torrent_label.clone(),
                                    std::borrow::ToOwned::to_owned,
                                );
                            root_ref.join(inferred)
                        },
                        PathBuf::from,
                    );

                    if let Some(parent) = destination.parent() {
                        fs::create_dir_all(parent).with_context(|| {
                            format!("failed to create destination parent {}", parent.display())
                        })?;
                    }

                    if destination.exists() {
                        if staging.canonicalize().ok() == destination.canonicalize().ok() {
                            meta.transfer_mode = Some(move_mode.to_owned());
                            meta.staging_path = Some(destination.to_string_lossy().into_owned());
                            meta.artifact_path = Some(destination.to_string_lossy().into_owned());
                            return Ok(StepOutcome::Skipped(Some(
                                "artifact already positioned".into(),
                            )));
                        }
                        if destination.is_file() {
                            fs::remove_file(&destination).with_context(|| {
                                format!(
                                    "failed to replace existing file at {}",
                                    destination.display()
                                )
                            })?;
                        } else {
                            fs::remove_dir_all(&destination).with_context(|| {
                                format!(
                                    "failed to replace existing directory at {}",
                                    destination.display()
                                )
                            })?;
                        }
                    }

                    match move_mode {
                        "copy" => Self::copy_tree(&staging, &destination)?,
                        "move" => Self::move_tree(&staging, &destination)?,
                        "hardlink" => Self::hardlink_tree(&staging, &destination)?,
                        other => bail!("unsupported FsOps move_mode '{other}'"),
                    }

                    meta.transfer_mode = Some(move_mode.to_owned());
                    meta.artifact_path = Some(destination.to_string_lossy().into_owned());
                    meta.staging_path = meta.artifact_path.clone();
                    Ok(StepOutcome::Completed(Some(format!(
                        "destination={}",
                        destination.display()
                    ))))
                }
            },
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::SetPermissions,
            StepPersistence::new(false, true, false),
            {
                let chmod_file = policy.chmod_file.clone();
                let chmod_dir = policy.chmod_dir.clone();
                let owner = policy.owner.clone();
                let group = policy.group.clone();
                let umask = policy.umask.clone();
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
                }
            },
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
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
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::Finalise,
            StepPersistence::new(true, true, false),
            |meta| {
                if let Some(work_dir) = meta.work_dir.as_ref().map(PathBuf::from)
                    && work_dir.exists()
                {
                    fs::remove_dir_all(&work_dir).ok();
                }
                meta.completed = true;
                meta.updated_at = Utc::now();
                let detail = meta.artifact_path.as_ref().map_or_else(
                    || "artifact=<unset>".to_string(),
                    |path| format!("artifact={path}"),
                );
                Ok(StepOutcome::Completed(Some(detail)))
            },
        )?;

        Ok(())
    }

    fn emit_progress(&self, torrent_id: Uuid, step: &str) {
        let _ = self.events.publish(Event::FsopsProgress {
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
    ) -> Result<FsOpsMeta> {
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
    ) -> Result<()>
    where
        F: FnOnce(&mut FsOpsMeta) -> Result<StepOutcome>,
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
                let _ = self.record_step(
                    meta,
                    meta_path,
                    step,
                    StepStatus::Failed,
                    Some(&detail),
                    persistence.failure,
                );
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
    ) -> Result<()> {
        let changed = meta.update_step(step, status, detail.map(str::to_string));
        if changed {
            if persist {
                persist_meta(meta_path, meta)?;
            }
            self.metrics.inc_fsops_step(step.as_str(), status.as_str());
        }
        Ok(())
    }

    fn extract_archive(source: &Path, target: &Path) -> Result<()> {
        let extension = source
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_lowercase);

        match extension.as_deref() {
            Some("zip") => Self::extract_zip(source, target),
            Some(other) => bail!("unsupported archive format '{other}'"),
            None => bail!(
                "unsupported archive without extension at {}",
                source.display()
            ),
        }
    }

    fn extract_zip(source: &Path, target: &Path) -> Result<()> {
        let file = File::open(source).with_context(|| {
            format!("failed to open archive {} for extraction", source.display())
        })?;
        let mut archive = ZipArchive::new(file)
            .with_context(|| format!("failed to decode zip archive {}", source.display()))?;

        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).with_context(|| {
                format!("failed to read entry {index} from {}", source.display())
            })?;
            let entry_path = Self::sanitize_archive_path(entry.name())?;
            let mut destination = target.to_path_buf();
            destination.push(&entry_path);

            if entry.name().ends_with('/') {
                fs::create_dir_all(&destination).with_context(|| {
                    format!(
                        "failed to create extracted directory {}",
                        destination.display()
                    )
                })?;
                continue;
            }

            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("failed to prepare extraction parent {}", parent.display())
                })?;
            }

            let mut output = File::create(&destination).with_context(|| {
                format!("failed to create extracted file {}", destination.display())
            })?;
            io::copy(&mut entry, &mut output)
                .with_context(|| format!("failed to extract file {}", destination.display()))?;

            #[cfg(unix)]
            if let Some(mode) = entry.unix_mode() {
                let perms = fs::Permissions::from_mode(mode);
                fs::set_permissions(&destination, perms).with_context(|| {
                    format!(
                        "failed to apply extracted file permissions to {}",
                        destination.display()
                    )
                })?;
            }
        }

        Ok(())
    }

    fn sanitize_archive_path(entry: &str) -> Result<PathBuf> {
        let path = Path::new(entry);
        ensure!(
            !path.is_absolute(),
            "archive entry '{entry}' may not be absolute"
        );

        let mut sanitized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::Normal(segment) => sanitized.push(segment),
                Component::CurDir => {}
                _ => bail!("archive entry '{entry}' contains invalid segments"),
            }
        }

        Ok(sanitized)
    }

    fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
        if source.is_file() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create destination parent {}", parent.display())
                })?;
            }
            fs::copy(source, destination).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source.display(),
                    destination.display()
                )
            })?;
            return Ok(());
        }

        fs::create_dir_all(destination).with_context(|| {
            format!(
                "failed to create destination directory {}",
                destination.display()
            )
        })?;

        for entry in WalkDir::new(source) {
            let entry = entry.with_context(|| {
                format!("failed to traverse {} while copying tree", source.display())
            })?;
            let relative = entry.path().strip_prefix(source).with_context(|| {
                format!(
                    "failed to strip prefix {} from {}",
                    source.display(),
                    entry.path().display()
                )
            })?;
            let target_path = destination.join(relative);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&target_path).with_context(|| {
                    format!(
                        "failed to create destination directory {}",
                        target_path.display()
                    )
                })?;
            } else {
                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent).with_context(|| {
                        format!("failed to create destination parent {}", parent.display())
                    })?;
                }
                fs::copy(entry.path(), &target_path).with_context(|| {
                    format!(
                        "failed to copy {} to {}",
                        entry.path().display(),
                        target_path.display()
                    )
                })?;
            }
        }

        Ok(())
    }

    fn move_tree(source: &Path, destination: &Path) -> Result<()> {
        match fs::rename(source, destination) {
            Ok(()) => Ok(()),
            Err(_rename_err) => {
                Self::copy_tree(source, destination).with_context(|| {
                    format!(
                        "failed to move {}; copy fallback also failed",
                        source.display()
                    )
                })?;
                fs::remove_dir_all(source)
                    .or_else(|_| fs::remove_file(source))
                    .with_context(|| {
                        format!(
                            "failed to remove source path {} after fallback copy",
                            source.display()
                        )
                    })?;
                Ok(())
            }
        }
    }

    fn hardlink_tree(source: &Path, destination: &Path) -> Result<()> {
        if source.is_file() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create destination parent {}", parent.display())
                })?;
            }
            fs::hard_link(source, destination).with_context(|| {
                format!(
                    "failed to hardlink {} to {}",
                    source.display(),
                    destination.display()
                )
            })?;
            return Ok(());
        }

        fs::create_dir_all(destination).with_context(|| {
            format!(
                "failed to create destination directory {}",
                destination.display()
            )
        })?;

        for entry in WalkDir::new(source) {
            let entry = entry.with_context(|| {
                format!(
                    "failed to traverse {} while hardlinking tree",
                    source.display()
                )
            })?;
            let relative = entry.path().strip_prefix(source).with_context(|| {
                format!(
                    "failed to strip prefix {} from {}",
                    source.display(),
                    entry.path().display()
                )
            })?;
            let target_path = destination.join(relative);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&target_path).with_context(|| {
                    format!(
                        "failed to create destination directory {}",
                        target_path.display()
                    )
                })?;
            } else {
                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent).with_context(|| {
                        format!("failed to create destination parent {}", parent.display())
                    })?;
                }
                fs::hard_link(entry.path(), &target_path).with_context(|| {
                    format!(
                        "failed to hardlink {} to {}",
                        entry.path().display(),
                        target_path.display()
                    )
                })?;
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
    ) -> Result<String> {
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
            bail!(
                "filesystem permission adjustments ({}) require a Unix-like platform",
                requested.join(", ")
            );
        }

        #[cfg(unix)]
        {
            let file_spec = file_mode
                .map(Self::parse_octal_mode)
                .transpose()
                .context("invalid file chmod specification")?;
            let dir_spec = dir_mode
                .map(Self::parse_octal_mode)
                .transpose()
                .context("invalid directory chmod specification")?;
            let umask_spec = umask
                .map(Self::parse_octal_mode)
                .transpose()
                .context("invalid umask specification")?;

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
                let entry = entry.with_context(|| {
                    format!(
                        "failed to traverse {} while adjusting permissions",
                        destination.display()
                    )
                })?;
                let path = entry.path();
                if entry.file_type().is_dir() {
                    if let Some((mode, _)) = dir_mode {
                        let perms = fs::Permissions::from_mode(mode);
                        fs::set_permissions(path, perms).with_context(|| {
                            format!(
                                "failed to apply directory permissions to {}",
                                path.display()
                            )
                        })?;
                    }
                } else if let Some((mode, _)) = file_mode {
                    let perms = fs::Permissions::from_mode(mode);
                    fs::set_permissions(path, perms).with_context(|| {
                        format!("failed to apply file permissions to {}", path.display())
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
    ) -> Result<Vec<String>> {
        let owner = owner.map(Self::resolve_owner).transpose()?;
        let group = group.map(Self::resolve_group).transpose()?;
        if owner.is_none() && group.is_none() {
            return Ok(Vec::new());
        }

        let uid = owner.as_ref().map(|(uid, _)| *uid);
        let gid = group.as_ref().map(|(gid, _)| *gid);

        for entry in WalkDir::new(destination) {
            let entry = entry.with_context(|| {
                format!(
                    "failed to traverse {} while adjusting ownership",
                    destination.display()
                )
            })?;
            let path = entry.path();
            chown(path, uid, gid)
                .with_context(|| format!("failed to apply ownership to {}", path.display()))?;
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
    ) -> Result<Vec<String>> {
        if owner.is_some() || group.is_some() {
            bail!("ownership adjustments require a Unix-like platform");
        }
        Ok(Vec::new())
    }

    #[cfg(unix)]
    fn resolve_owner(spec: &str) -> Result<(Uid, String)> {
        let trimmed = spec.trim();
        ensure!(!trimmed.is_empty(), "owner specification cannot be empty");
        if let Ok(id) = trimmed.parse::<u32>() {
            let uid = Uid::from_raw(id);
            return Ok((uid, format!("uid({id})")));
        }
        let user = User::from_name(trimmed)
            .with_context(|| format!("failed to resolve user '{trimmed}'"))?
            .ok_or_else(|| anyhow!("user '{trimmed}' not found"))?;
        Ok((user.uid, format!("{trimmed}({})", user.uid.as_raw())))
    }

    #[cfg(unix)]
    fn resolve_group(spec: &str) -> Result<(Gid, String)> {
        let trimmed = spec.trim();
        ensure!(!trimmed.is_empty(), "group specification cannot be empty");
        if let Ok(id) = trimmed.parse::<u32>() {
            let gid = Gid::from_raw(id);
            return Ok((gid, format!("gid({id})")));
        }
        let group = Group::from_name(trimmed)
            .with_context(|| format!("failed to resolve group '{trimmed}'"))?
            .ok_or_else(|| anyhow!("group '{trimmed}' not found"))?;
        Ok((group.gid, format!("{trimmed}({})", group.gid.as_raw())))
    }

    fn cleanup_destination(destination: &Path, rules: &RuleSet) -> usize {
        let mut removed = 0usize;

        let mut files = Vec::new();
        let mut directories = Vec::new();
        for entry in WalkDir::new(destination).into_iter().filter_map(Result::ok) {
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
                RuleDecision::Skip => {
                    if fs::remove_file(entry.path()).is_ok() {
                        removed += 1;
                    }
                }
            }
        }

        directories.sort_by_key(walkdir::DirEntry::depth);
        directories.reverse();

        for entry in directories {
            match rules.evaluate(entry.path()) {
                RuleDecision::Include => {}
                RuleDecision::Skip => {
                    if entry
                        .path()
                        .read_dir()
                        .map(|mut iter| iter.next().is_none())
                        .unwrap_or(false)
                    {
                        fs::remove_dir(entry.path()).ok();
                    }
                }
            }
        }

        removed
    }

    fn parse_octal_mode(value: &str) -> Result<u32> {
        let trimmed = value.trim_start_matches("0o");
        u32::from_str_radix(trimmed, 8).context("invalid octal mode")
    }

    fn mark_degraded(&self, detail: &str) {
        let mut guard = self
            .health_degraded
            .lock()
            .expect("fsops health mutex poisoned");
        if *guard {
            drop(guard);
            warn!(
                component = HEALTH_COMPONENT,
                "fsops pipeline still degraded: {detail}"
            );
        } else {
            *guard = true;
            drop(guard);
            warn!(
                component = HEALTH_COMPONENT,
                "fsops pipeline degraded: {detail}"
            );
            let _ = self.events.publish(Event::HealthChanged {
                degraded: vec![HEALTH_COMPONENT.to_string()],
            });
        }
    }

    fn mark_recovered(&self) {
        let mut guard = self
            .health_degraded
            .lock()
            .expect("fsops health mutex poisoned");
        if std::mem::take(&mut *guard) {
            drop(guard);
            let _ = self
                .events
                .publish(Event::HealthChanged { degraded: vec![] });
            info!(component = HEALTH_COMPONENT, "fsops pipeline recovered");
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

fn load_meta(path: &Path) -> Result<FsOpsMeta> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read fsops metadata file at {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse fsops metadata JSON at {}", path.display()))
}

fn persist_meta(path: &Path, meta: &FsOpsMeta) -> Result<()> {
    let serialised = serde_json::to_string_pretty(meta)
        .context("failed to serialise fsops metadata for persistence")?;
    fs::write(path, serialised)
        .with_context(|| format!("failed to persist fsops metadata at {}", path.display()))
}

fn enforce_allow_paths(root: &Path, allow_paths: &Value) -> Result<()> {
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

    ensure!(
        permitted,
        "library root {} is not permitted by fs policy allow_paths",
        root_abs.display()
    );

    Ok(())
}

fn parse_path_list(value: &Value) -> Result<Vec<PathBuf>> {
    match value {
        Value::Array(entries) => entries
            .iter()
            .map(|entry| match entry {
                Value::String(path) if !path.trim().is_empty() => Ok(PathBuf::from(path)),
                Value::String(_) => Err(anyhow!("allow path entries cannot be empty")),
                other => Err(anyhow!(
                    "allow path entries must be strings (found {other:?})"
                )),
            })
            .collect(),
        Value::Null => Ok(Vec::new()),
        Value::Object(obj) if obj.is_empty() => Ok(Vec::new()),
        other => Err(anyhow!(
            "allow_paths must be an array of strings (found {other:?})"
        )),
    }
}

#[derive(Debug)]
struct RuleSet {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
}

impl RuleSet {
    fn from_policy(policy: &FsPolicy) -> Result<Self> {
        let include_patterns = parse_glob_list(&policy.cleanup_keep)?;
        let mut exclude_patterns = parse_glob_list(&policy.cleanup_drop)?;

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
            include: build_globset(include_patterns)?,
            exclude: build_globset(exclude_patterns)?,
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
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

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, PartialEq, Eq)]
enum RuleDecision {
    Include,
    Skip,
}

fn parse_glob_list(value: &Value) -> Result<Vec<String>> {
    match value {
        Value::Array(entries) => entries
            .iter()
            .map(|entry| match entry {
                Value::String(pattern) if !pattern.trim().is_empty() => Ok(pattern.to_string()),
                Value::String(_) => Err(anyhow!("glob patterns cannot be empty strings")),
                other => Err(anyhow!("glob patterns must be strings (found {other:?})")),
            })
            .collect(),
        Value::Null => Ok(Vec::new()),
        Value::Object(obj) if obj.is_empty() => Ok(Vec::new()),
        other => Err(anyhow!("expected array of glob patterns (found {other:?})")),
    }
}

fn build_globset(patterns: Vec<String>) -> Result<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(
            Glob::new(&pattern)
                .with_context(|| format!("invalid glob pattern '{pattern}' in fsops policy"))?,
        );
    }
    Ok(Some(
        builder.build().context("failed to compile glob patterns")?,
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::redundant_clone)]

    use super::*;
    use revaer_events::EventBus;
    use serde_json::json;
    use tempfile::TempDir;
    use tokio::time::{Duration, timeout};

    fn sample_policy(root: &Path) -> FsPolicy {
        let library_root = root.join("library");
        FsPolicy {
            id: Uuid::new_v4(),
            library_root: library_root.display().to_string(),
            extract: false,
            par2: "disabled".to_string(),
            flatten: false,
            move_mode: "copy".to_string(),
            cleanup_keep: json!(["**/*.mkv"]),
            cleanup_drop: json!([]),
            chmod_file: None,
            chmod_dir: None,
            owner: None,
            group: None,
            umask: None,
            allow_paths: json!([root.display().to_string()]),
        }
    }

    async fn collect_events(stream: &mut revaer_events::EventStream, count: usize) -> Vec<Event> {
        let mut events = Vec::new();
        for _ in 0..count {
            match timeout(Duration::from_secs(2), stream.next()).await {
                Ok(Some(envelope)) => events.push(envelope.event),
                _ => break,
            }
        }
        events
    }

    #[tokio::test]
    async fn pipeline_transfers_files_and_persists_meta() -> Result<()> {
        let bus = EventBus::with_capacity(32);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus.clone(), metrics.clone());
        let mut stream = bus.subscribe(None);
        let torrent_id = Uuid::new_v4();
        let temp = TempDir::new()?;

        let staging_root = temp.path().join("staging");
        fs::create_dir_all(&staging_root)?;
        let source_dir = staging_root.join("torrent-files");
        fs::create_dir_all(&source_dir)?;
        fs::write(source_dir.join("movie.mkv"), b"video-bytes")?;
        fs::write(source_dir.join("junk.tmp"), b"junk")?;

        let policy = sample_policy(temp.path());
        let request = FsOpsRequest {
            torrent_id,
            source_path: &source_dir,
            policy: &policy,
        };
        service.apply(request)?;

        let events = collect_events(&mut stream, 16).await;
        assert!(matches!(events[0], Event::FsopsStarted { torrent_id: id } if id == torrent_id));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::FsopsCompleted { torrent_id: id } if *id == torrent_id
        )));

        let meta_path = Path::new(&policy.library_root)
            .join(META_DIR_NAME)
            .join(format!("{torrent_id}{META_SUFFIX}"));
        assert!(meta_path.exists(), "meta file should be persisted");
        let meta = load_meta(&meta_path)?;
        assert!(meta.completed);
        let artifact = meta
            .artifact_path
            .as_ref()
            .expect("artifact path recorded in meta");
        let artifact_path = PathBuf::from(artifact);
        assert!(artifact_path.join("movie.mkv").exists());
        assert!(
            !artifact_path.join("junk.tmp").exists(),
            "cleanup rules should remove non-matching files"
        );

        let rendered = metrics.render()?;
        assert!(
            rendered.contains(r#"fsops_steps_total{status="completed",step="finalise"} 1"#),
            "expected fsops finalise metric to increment"
        );

        Ok(())
    }

    #[tokio::test]
    async fn pipeline_is_idempotent_when_meta_completed() -> Result<()> {
        let bus = EventBus::with_capacity(16);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus.clone(), metrics);
        let torrent_id = Uuid::new_v4();
        let temp = TempDir::new()?;
        let policy = sample_policy(temp.path());

        let staging_root = temp.path().join("staging");
        fs::create_dir_all(&staging_root)?;
        let source_dir = staging_root.join("torrent-files");
        fs::create_dir_all(&source_dir)?;
        fs::write(source_dir.join("movie.mkv"), b"video-v1")?;

        let request = FsOpsRequest {
            torrent_id,
            source_path: &source_dir,
            policy: &policy,
        };

        service.apply(request)?;
        let meta_path = Path::new(&policy.library_root)
            .join(META_DIR_NAME)
            .join(format!("{torrent_id}{META_SUFFIX}"));
        let meta_before = load_meta(&meta_path)?;

        service.apply(FsOpsRequest {
            torrent_id,
            source_path: &source_dir,
            policy: &policy,
        })?;
        let meta_after = load_meta(&meta_path)?;

        assert_eq!(meta_before.updated_at, meta_after.updated_at);
        assert!(meta_after.completed);
        Ok(())
    }

    #[tokio::test]
    async fn pipeline_resumes_incomplete_meta() -> Result<()> {
        let bus = EventBus::with_capacity(16);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus.clone(), metrics);
        let torrent_id = Uuid::new_v4();
        let temp = TempDir::new()?;
        let policy = sample_policy(temp.path());

        let staging_root = temp.path().join("staging");
        fs::create_dir_all(&staging_root)?;
        let source_dir = staging_root.join("torrent-files");
        fs::create_dir_all(&source_dir)?;
        fs::write(source_dir.join("movie.mkv"), b"video-v1")?;

        service.apply(FsOpsRequest {
            torrent_id,
            source_path: &source_dir,
            policy: &policy,
        })?;
        let meta_path = Path::new(&policy.library_root)
            .join(META_DIR_NAME)
            .join(format!("{torrent_id}{META_SUFFIX}"));
        let mut meta = load_meta(&meta_path)?;
        meta.completed = false;
        persist_meta(&meta_path, &meta)?;

        service.apply(FsOpsRequest {
            torrent_id,
            source_path: &source_dir,
            policy: &policy,
        })?;
        let repaired = load_meta(&meta_path)?;
        assert!(repaired.completed);
        Ok(())
    }

    #[tokio::test]
    async fn pipeline_enforces_allowlist() {
        let bus = EventBus::with_capacity(16);
        let metrics = Metrics::new().expect("metrics");
        let service = FsOpsService::new(bus.clone(), metrics);
        let mut stream = bus.subscribe(None);
        let torrent_id = Uuid::new_v4();
        let temp = TempDir::new().expect("tempdir");
        let staging_root = temp.path().join("staging");
        fs::create_dir_all(&staging_root).expect("staging");
        let source_dir = staging_root.join("torrent-files");
        fs::create_dir_all(&source_dir).expect("source");
        fs::write(source_dir.join("movie.mkv"), b"video").expect("write");

        let mut policy = sample_policy(temp.path());
        policy.allow_paths = json!([temp.path().join("disallowed").display().to_string()]);

        let result = service.apply(FsOpsRequest {
            torrent_id,
            source_path: &source_dir,
            policy: &policy,
        });
        assert!(result.is_err(), "allowlist violation should fail");

        let events = collect_events(&mut stream, 16).await;
        assert!(events.iter().any(|event| matches!(
            event,
            Event::FsopsFailed { torrent_id: id, .. } if *id == torrent_id
        )));
        assert!(events.iter().any(|event| matches!(event, Event::HealthChanged { degraded } if degraded.contains(&HEALTH_COMPONENT.to_string()))));
    }

    #[tokio::test]
    async fn pipeline_marks_degraded_and_recovers() -> Result<()> {
        let bus = EventBus::with_capacity(16);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus.clone(), metrics);
        let mut stream = bus.subscribe(None);
        let torrent_id = Uuid::new_v4();
        let temp = TempDir::new()?;
        let staging_root = temp.path().join("staging");
        fs::create_dir_all(&staging_root)?;
        let source_dir = staging_root.join("torrent-files");
        fs::create_dir_all(&source_dir)?;
        fs::write(source_dir.join("movie.mkv"), b"video")?;

        let mut invalid = sample_policy(temp.path());
        invalid.library_root = " ".to_string();

        let err = service
            .apply(FsOpsRequest {
                torrent_id,
                source_path: &source_dir,
                policy: &invalid,
            })
            .expect_err("invalid policy should fail");
        assert!(
            format!("{err:#}").contains("cannot be empty"),
            "unexpected error: {err:?}"
        );

        let failure_events = collect_events(&mut stream, 4).await;
        assert!(failure_events.iter().any(|event| matches!(
            event,
            Event::FsopsStarted { torrent_id: id } if *id == torrent_id
        )));
        assert!(failure_events.iter().any(|event| matches!(
            event,
            Event::HealthChanged { degraded } if degraded.contains(&HEALTH_COMPONENT.to_string())
        )));
        assert!(failure_events.iter().any(|event| matches!(
            event,
            Event::FsopsFailed { torrent_id: id, .. } if *id == torrent_id
        )));

        let valid = sample_policy(temp.path());
        service.apply(FsOpsRequest {
            torrent_id,
            source_path: &source_dir,
            policy: &valid,
        })?;
        let recovery_events = collect_events(&mut stream, 16).await;
        assert!(recovery_events.iter().any(|event| matches!(
            event,
            Event::FsopsCompleted { torrent_id: id } if *id == torrent_id
        )));
        assert!(recovery_events.iter().any(|event| matches!(
            event,
            Event::HealthChanged { degraded } if degraded.is_empty()
        )));

        Ok(())
    }

    #[test]
    fn skip_fluff_preset_extends_patterns() -> Result<()> {
        let policy = FsPolicy {
            id: Uuid::new_v4(),
            library_root: "/tmp/library".to_string(),
            extract: false,
            par2: "disabled".to_string(),
            flatten: false,
            move_mode: "copy".to_string(),
            cleanup_keep: json!([]),
            cleanup_drop: json!([SKIP_FLUFF_PRESET]),
            chmod_file: None,
            chmod_dir: None,
            owner: None,
            group: None,
            umask: None,
            allow_paths: json!([]),
        };

        let rules = RuleSet::from_policy(&policy)?;
        assert_eq!(rules.exclude_count(), SKIP_FLUFF_PATTERNS.len());
        Ok(())
    }

    #[test]
    fn rule_set_evaluates_include_and_exclude() -> Result<()> {
        let policy = FsPolicy {
            id: Uuid::new_v4(),
            library_root: "/tmp/library".to_string(),
            extract: false,
            par2: "disabled".to_string(),
            flatten: false,
            move_mode: "copy".to_string(),
            cleanup_keep: json!(["**/*.mkv"]),
            cleanup_drop: json!(["**/extras/**"]),
            chmod_file: None,
            chmod_dir: None,
            owner: None,
            group: None,
            umask: None,
            allow_paths: json!([]),
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
    fn parse_path_list_rejects_invalid_entries() {
        let values = json!(["", {"path": "/tmp"}]);
        let err = parse_path_list(&values).expect_err("invalid inputs should fail");
        assert!(format!("{err:#}").contains("allow path entries"));
    }

    #[test]
    fn parse_glob_list_rejects_non_strings() {
        let values = json!({"pattern": "**/*.mkv"});
        let err = parse_glob_list(&values).expect_err("non-array should fail");
        assert!(format!("{err:#}").contains("expected array"));
    }

    #[tokio::test]
    async fn pipeline_flattens_single_directory() -> Result<()> {
        let bus = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus, metrics);
        let torrent_id = Uuid::new_v4();
        let temp = TempDir::new()?;

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
        let artifact = PathBuf::from(meta.artifact_path.as_ref().expect("artifact path set"));
        assert!(artifact.ends_with("Season1"));
        assert!(artifact.join("episode.mkv").exists());

        Ok(())
    }

    #[test]
    fn enforce_allow_paths_accepts_parent_directory() -> Result<()> {
        let temp = TempDir::new()?;
        let root = temp.path().join("library");
        let allow = json!([temp.path().display().to_string()]);
        enforce_allow_paths(&root, &allow)?;
        Ok(())
    }
}
