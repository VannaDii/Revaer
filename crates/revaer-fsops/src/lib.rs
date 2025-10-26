use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow, ensure};
use chrono::{DateTime, Utc};
use globset::{Glob, GlobSet, GlobSetBuilder};
use revaer_config::FsPolicy;
use revaer_events::{Event, EventBus};
use revaer_telemetry::Metrics;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, warn};
use uuid::Uuid;

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
    Finalise,
}

impl StepKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ValidatePolicy => "validate_policy",
            Self::Allowlist => "allowlist",
            Self::PrepareDirectories => "prepare_directories",
            Self::CompileRules => "compile_rules",
            Self::Finalise => "finalise",
        }
    }

    const fn progress_label(self) -> &'static str {
        match self {
            Self::ValidatePolicy => "validate",
            Self::Allowlist => "allowlist",
            Self::PrepareDirectories => "prepare_directories",
            Self::CompileRules => "compile_rules",
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
}

impl StepStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Failed => "failed",
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
    pub fn apply_policy(&self, torrent_id: Uuid, policy: &FsPolicy) -> Result<()> {
        let _ = self.events.publish(Event::FsopsStarted { torrent_id });

        let result = self.execute_pipeline(torrent_id, policy);

        match &result {
            Ok(()) => {
                self.mark_recovered();
                let _ = self.events.publish(Event::FsopsCompleted { torrent_id });
            }
            Err(error) => {
                self.mark_degraded(&format!("{error:#}"));
                let _ = self.events.publish(Event::FsopsFailed {
                    torrent_id,
                    message: format!("{error:#}"),
                });
            }
        }

        result
    }

    fn execute_pipeline(&self, torrent_id: Uuid, policy: &FsPolicy) -> Result<()> {
        let root = PathBuf::from(&policy.library_root);
        let meta_dir = root.join(META_DIR_NAME);
        let meta_path = meta_dir.join(format!("{torrent_id}{META_SUFFIX}"));

        let mut meta = self.load_or_initialise_meta(torrent_id, policy.id, &meta_path)?;

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
            |_| {
                ensure!(
                    !policy.library_root.trim().is_empty(),
                    "filesystem policy library root cannot be empty"
                );
                Ok(())
            },
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::Allowlist,
            StepPersistence::new(false, false, false),
            |_| {
                enforce_allow_paths(&root, &policy.allow_paths)?;
                Ok(())
            },
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::PrepareDirectories,
            StepPersistence::new(false, true, false),
            |_| {
                fs::create_dir_all(&root).with_context(|| {
                    format!(
                        "failed to create library root directory at {}",
                        root.display()
                    )
                })?;
                fs::create_dir_all(&meta_dir).with_context(|| {
                    format!(
                        "failed to create fsops metadata directory at {}",
                        meta_dir.display()
                    )
                })?;
                Ok(())
            },
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::CompileRules,
            StepPersistence::new(true, true, true),
            |_| {
                let _ = RuleSet::from_policy(policy)?;
                Ok(())
            },
        )?;

        self.execute_step(
            torrent_id,
            &mut meta,
            &meta_path,
            StepKind::Finalise,
            StepPersistence::new(true, true, true),
            |meta| {
                meta.completed = true;
                meta.updated_at = Utc::now();
                Ok(())
            },
        )?;

        let rules = RuleSet::from_policy(policy)?;
        info!(
            torrent_id = %torrent_id,
            library_root = %root.display(),
            include_rules = rules.include_count(),
            exclude_rules = rules.exclude_count(),
            "filesystem post-processing pipeline completed"
        );
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
    ) -> Result<FsOpsMeta> {
        if meta_path.exists() {
            self.emit_progress(torrent_id, "load_meta");
            load_meta(meta_path)
        } else {
            Ok(FsOpsMeta::new(torrent_id, policy_id))
        }
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
        F: FnOnce(&mut FsOpsMeta) -> Result<()>,
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
            Ok(()) => {
                self.record_step(
                    meta,
                    meta_path,
                    step,
                    StepStatus::Completed,
                    None,
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
}

impl FsOpsMeta {
    fn new(torrent_id: Uuid, policy_id: Uuid) -> Self {
        Self {
            torrent_id,
            policy_id,
            completed: false,
            updated_at: Utc::now(),
            steps: Vec::new(),
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
            match timeout(Duration::from_millis(500), stream.next()).await {
                Ok(Some(envelope)) => events.push(envelope.event),
                _ => break,
            }
        }
        events
    }

    #[tokio::test]
    async fn pipeline_emits_lifecycle_and_persists_meta() -> Result<()> {
        let bus = EventBus::with_capacity(32);
        let metrics = Metrics::new()?;
        let service = FsOpsService::new(bus.clone(), metrics.clone());
        let mut stream = bus.subscribe(None);
        let torrent_id = Uuid::new_v4();
        let temp = TempDir::new()?;
        let root = temp.path().join("library");
        let policy = sample_policy(temp.path());
        service.apply_policy(torrent_id, &policy)?;

        let events = collect_events(&mut stream, 8).await;
        assert!(matches!(events[0], Event::FsopsStarted { torrent_id: id } if id == torrent_id));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::FsopsProgress { torrent_id: id, step } if *id == torrent_id && step == "finalise"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            Event::FsopsCompleted { torrent_id: id } if *id == torrent_id
        )));

        let meta_path = root
            .join(META_DIR_NAME)
            .join(format!("{torrent_id}{META_SUFFIX}"));
        assert!(meta_path.exists(), "meta file should be persisted");
        let meta = load_meta(&meta_path)?;
        assert!(meta.completed);
        assert_eq!(
            meta.step_status(StepKind::CompileRules),
            Some(StepStatus::Completed)
        );
        assert_eq!(
            meta.step_status(StepKind::Finalise),
            Some(StepStatus::Completed)
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
        let root = temp.path().join("library");
        let policy = sample_policy(temp.path());

        service.apply_policy(torrent_id, &policy)?;
        let meta_path = root
            .join(META_DIR_NAME)
            .join(format!("{torrent_id}{META_SUFFIX}"));
        let meta_before = load_meta(&meta_path)?;

        service.apply_policy(torrent_id, &policy)?;
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
        let root = temp.path().join("library");
        let policy = sample_policy(temp.path());

        service.apply_policy(torrent_id, &policy)?;
        let meta_path = root
            .join(META_DIR_NAME)
            .join(format!("{torrent_id}{META_SUFFIX}"));
        let mut meta = load_meta(&meta_path)?;
        meta.completed = false;
        persist_meta(&meta_path, &meta)?;

        service.apply_policy(torrent_id, &policy)?;
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
        let root = temp.path().join("library");
        let mut policy = sample_policy(temp.path());
        policy.allow_paths = json!([root.join("disallowed").display().to_string()]);

        let result = service.apply_policy(torrent_id, &policy);
        assert!(result.is_err(), "allowlist violation should fail");

        let events = collect_events(&mut stream, 8).await;
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

        let mut invalid = sample_policy(temp.path());
        invalid.library_root = " ".to_string();

        let err = service
            .apply_policy(torrent_id, &invalid)
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
        service.apply_policy(torrent_id, &valid)?;
        let recovery_events = collect_events(&mut stream, 8).await;
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

    #[test]
    fn enforce_allow_paths_accepts_parent_directory() -> Result<()> {
        let temp = TempDir::new()?;
        let root = temp.path().join("library");
        let allow = json!([temp.path().display().to_string()]);
        enforce_allow_paths(&root, &allow)?;
        Ok(())
    }
}
