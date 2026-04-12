//! In-process indexer maintenance runtime.
//!
//! # Design
//! - Runs lightweight scheduled indexer maintenance jobs from the application process.
//! - Keeps infrastructure injected via a backend trait so tests stay host-only and deterministic.
//! - Treats stable claim errors like `job_not_due` as skips while surfacing unexpected failures.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use revaer_config::ConfigService;
use revaer_data::DataError;
use revaer_data::indexers::jobs::{
    job_claim_next, job_run_base_score_refresh_recent, job_run_canonical_backfill_best_source,
    job_run_canonical_prune_low_confidence, job_run_connectivity_profile_refresh,
    job_run_policy_snapshot_gc, job_run_policy_snapshot_refcount_repair,
    job_run_rate_limit_state_purge, job_run_reputation_rollup, job_run_retention_purge,
    job_run_rss_subscription_backfill,
};
use revaer_telemetry::Metrics;
use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{info, warn};

const DEFAULT_TICK_INTERVAL: Duration = Duration::from_secs(1);
const CLAIM_SKIP_CODE: &str = "P0001";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobKind {
    RetentionPurge,
    ConnectivityProfileRefresh,
    ReputationRollup1h,
    ReputationRollup24h,
    ReputationRollup7d,
    CanonicalBackfillBestSource,
    CanonicalPruneLowConfidence,
    BaseScoreRefreshRecent,
    PolicySnapshotGc,
    PolicySnapshotRefcountRepair,
    RateLimitStatePurge,
    RssSubscriptionBackfill,
}

impl JobKind {
    const ALL: [Self; 12] = [
        Self::RetentionPurge,
        Self::ConnectivityProfileRefresh,
        Self::ReputationRollup1h,
        Self::ReputationRollup24h,
        Self::ReputationRollup7d,
        Self::CanonicalBackfillBestSource,
        Self::CanonicalPruneLowConfidence,
        Self::BaseScoreRefreshRecent,
        Self::PolicySnapshotGc,
        Self::PolicySnapshotRefcountRepair,
        Self::RateLimitStatePurge,
        Self::RssSubscriptionBackfill,
    ];

    const fn job_key(self) -> &'static str {
        match self {
            Self::RetentionPurge => "retention_purge",
            Self::ConnectivityProfileRefresh => "connectivity_profile_refresh",
            Self::ReputationRollup1h => "reputation_rollup_1h",
            Self::ReputationRollup24h => "reputation_rollup_24h",
            Self::ReputationRollup7d => "reputation_rollup_7d",
            Self::CanonicalBackfillBestSource => "canonical_backfill_best_source",
            Self::CanonicalPruneLowConfidence => "canonical_prune_low_confidence",
            Self::BaseScoreRefreshRecent => "base_score_refresh_recent",
            Self::PolicySnapshotGc => "policy_snapshot_gc",
            Self::PolicySnapshotRefcountRepair => "policy_snapshot_refcount_repair",
            Self::RateLimitStatePurge => "rate_limit_state_purge",
            Self::RssSubscriptionBackfill => "rss_subscription_backfill",
        }
    }

    const fn operation(self) -> &'static str {
        match self {
            Self::RetentionPurge => "indexer_runtime.retention_purge",
            Self::ConnectivityProfileRefresh => "indexer_runtime.connectivity_profile_refresh",
            Self::ReputationRollup1h => "indexer_runtime.reputation_rollup_1h",
            Self::ReputationRollup24h => "indexer_runtime.reputation_rollup_24h",
            Self::ReputationRollup7d => "indexer_runtime.reputation_rollup_7d",
            Self::CanonicalBackfillBestSource => "indexer_runtime.canonical_backfill_best_source",
            Self::CanonicalPruneLowConfidence => "indexer_runtime.canonical_prune_low_confidence",
            Self::BaseScoreRefreshRecent => "indexer_runtime.base_score_refresh_recent",
            Self::PolicySnapshotGc => "indexer_runtime.policy_snapshot_gc",
            Self::PolicySnapshotRefcountRepair => "indexer_runtime.policy_snapshot_refcount_repair",
            Self::RateLimitStatePurge => "indexer_runtime.rate_limit_state_purge",
            Self::RssSubscriptionBackfill => "indexer_runtime.rss_subscription_backfill",
        }
    }
}

#[async_trait]
trait IndexerJobBackend: Send + Sync {
    async fn claim_next(&self, job_key: &'static str) -> Result<(), DataError>;
    async fn run_job(&self, job: JobKind) -> Result<(), DataError>;
}

struct StoredProcIndexerJobBackend {
    config: Arc<ConfigService>,
}

impl StoredProcIndexerJobBackend {
    const fn new(config: Arc<ConfigService>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl IndexerJobBackend for StoredProcIndexerJobBackend {
    async fn claim_next(&self, job_key: &'static str) -> Result<(), DataError> {
        job_claim_next(self.config.pool(), job_key).await
    }

    async fn run_job(&self, job: JobKind) -> Result<(), DataError> {
        match job {
            JobKind::RetentionPurge => job_run_retention_purge(self.config.pool()).await,
            JobKind::ConnectivityProfileRefresh => {
                job_run_connectivity_profile_refresh(self.config.pool()).await
            }
            JobKind::ReputationRollup1h => {
                job_run_reputation_rollup(self.config.pool(), "1h").await
            }
            JobKind::ReputationRollup24h => {
                job_run_reputation_rollup(self.config.pool(), "24h").await
            }
            JobKind::ReputationRollup7d => {
                job_run_reputation_rollup(self.config.pool(), "7d").await
            }
            JobKind::CanonicalBackfillBestSource => {
                job_run_canonical_backfill_best_source(self.config.pool()).await
            }
            JobKind::CanonicalPruneLowConfidence => {
                job_run_canonical_prune_low_confidence(self.config.pool()).await
            }
            JobKind::BaseScoreRefreshRecent => {
                job_run_base_score_refresh_recent(self.config.pool()).await
            }
            JobKind::PolicySnapshotGc => job_run_policy_snapshot_gc(self.config.pool()).await,
            JobKind::PolicySnapshotRefcountRepair => {
                job_run_policy_snapshot_refcount_repair(self.config.pool()).await
            }
            JobKind::RateLimitStatePurge => {
                job_run_rate_limit_state_purge(self.config.pool()).await
            }
            JobKind::RssSubscriptionBackfill => {
                job_run_rss_subscription_backfill(self.config.pool()).await
            }
        }
    }
}

pub(crate) struct IndexerRuntime {
    backend: Arc<dyn IndexerJobBackend>,
    telemetry: Metrics,
    tick_interval: Duration,
}

impl IndexerRuntime {
    pub(crate) fn new(config: Arc<ConfigService>, telemetry: Metrics) -> Self {
        Self::with_backend(
            Arc::new(StoredProcIndexerJobBackend::new(config)),
            telemetry,
            DEFAULT_TICK_INTERVAL,
        )
    }

    fn with_backend(
        backend: Arc<dyn IndexerJobBackend>,
        telemetry: Metrics,
        tick_interval: Duration,
    ) -> Self {
        Self {
            backend,
            telemetry,
            tick_interval,
        }
    }

    pub(crate) fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.run_loop().await;
        })
    }

    async fn run_loop(self) {
        let mut ticker = interval(self.tick_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            self.run_tick().await;
        }
    }

    async fn run_tick(&self) {
        for job in JobKind::ALL {
            self.run_job(job).await;
        }
    }

    async fn run_job(&self, job: JobKind) {
        let started = Instant::now();
        let operation = job.operation();
        let job_key = job.job_key();

        match self.backend.claim_next(job_key).await {
            Ok(()) => {}
            Err(error) if is_skip_claim_error(&error) => {
                self.record_job_outcome(operation, "skipped", started.elapsed());
                return;
            }
            Err(error) => {
                Self::record_job_error(job_key, "claim", &error);
                self.record_job_outcome(operation, "failure", started.elapsed());
                return;
            }
        }

        match self.backend.run_job(job).await {
            Ok(()) => {
                info!(job_key, operation, "indexer runtime job completed");
                self.record_job_outcome(operation, "success", started.elapsed());
            }
            Err(error) => {
                Self::record_job_error(job_key, "run", &error);
                self.record_job_outcome(operation, "failure", started.elapsed());
            }
        }
    }

    fn record_job_error(job_key: &'static str, phase: &'static str, error: &DataError) {
        warn!(
            job_key,
            phase,
            error_code = error.database_code().as_deref().unwrap_or(""),
            error_detail = error.database_detail().unwrap_or(""),
            error = %error,
            "indexer runtime job failed"
        );
    }

    fn record_job_outcome(
        &self,
        operation: &'static str,
        outcome: &'static str,
        elapsed: Duration,
    ) {
        self.telemetry.inc_indexer_job_outcome(operation, outcome);
        self.telemetry
            .observe_indexer_operation_latency(operation, outcome, elapsed);
    }
}

fn is_skip_claim_error(error: &DataError) -> bool {
    if error.database_code().as_deref() != Some(CLAIM_SKIP_CODE) {
        return false;
    }

    matches!(
        error.database_detail(),
        Some("job_not_due" | "job_locked" | "job_disabled")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};
    use std::sync::Mutex;
    use tokio::task::yield_now;
    use tokio::time::timeout;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum BackendCall {
        Claim(&'static str),
        Run(JobKind),
    }

    struct FakeIndexerJobBackend {
        claim_results: Mutex<VecDeque<Result<(), DataError>>>,
        run_results: Mutex<VecDeque<Result<(), DataError>>>,
        calls: Mutex<Vec<BackendCall>>,
    }

    impl FakeIndexerJobBackend {
        fn new(
            claim_results: Vec<Result<(), DataError>>,
            run_results: Vec<Result<(), DataError>>,
        ) -> Self {
            Self {
                claim_results: Mutex::new(VecDeque::from(claim_results)),
                run_results: Mutex::new(VecDeque::from(run_results)),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<BackendCall> {
            self.calls
                .lock()
                .map(|calls| calls.clone())
                .unwrap_or_default()
        }
    }

    #[async_trait]
    impl IndexerJobBackend for FakeIndexerJobBackend {
        async fn claim_next(&self, job_key: &'static str) -> Result<(), DataError> {
            if let Ok(mut calls) = self.calls.lock() {
                calls.push(BackendCall::Claim(job_key));
            }
            self.claim_results
                .lock()
                .ok()
                .and_then(|mut queue| queue.pop_front())
                .unwrap_or(Ok(()))
        }

        async fn run_job(&self, job: JobKind) -> Result<(), DataError> {
            if let Ok(mut calls) = self.calls.lock() {
                calls.push(BackendCall::Run(job));
            }
            self.run_results
                .lock()
                .ok()
                .and_then(|mut queue| queue.pop_front())
                .unwrap_or(Ok(()))
        }
    }

    fn job_not_due_error() -> DataError {
        DataError::JobFailed {
            operation: "job claim next",
            job_key: "retention_purge",
            error_code: Some(CLAIM_SKIP_CODE.to_string()),
            error_detail: Some("job_not_due".to_string()),
        }
    }

    #[test]
    fn skip_claim_errors_are_classified() {
        assert!(is_skip_claim_error(&job_not_due_error()));
        assert!(!is_skip_claim_error(&DataError::JobFailed {
            operation: "job claim next",
            job_key: "retention_purge",
            error_code: Some(CLAIM_SKIP_CODE.to_string()),
            error_detail: Some("job_not_found".to_string()),
        }));
        assert!(!is_skip_claim_error(&DataError::QueryFailed {
            operation: "job claim next",
            source: sqlx::Error::RowNotFound,
        }));
    }

    #[test]
    fn job_kind_metadata_is_unique_and_stable() {
        let mut job_keys = HashSet::new();
        let mut operations = HashSet::new();

        for job in JobKind::ALL {
            assert!(
                job_keys.insert(job.job_key()),
                "duplicate job key for {job:?}"
            );
            assert!(
                operations.insert(job.operation()),
                "duplicate operation for {job:?}"
            );
        }

        assert_eq!(job_keys.len(), JobKind::ALL.len());
        assert_eq!(operations.len(), JobKind::ALL.len());
        assert_eq!(JobKind::RetentionPurge.job_key(), "retention_purge");
        assert_eq!(
            JobKind::RssSubscriptionBackfill.operation(),
            "indexer_runtime.rss_subscription_backfill"
        );
    }

    #[tokio::test]
    async fn runtime_skips_not_due_jobs_without_running_them() -> anyhow::Result<()> {
        let backend = Arc::new(FakeIndexerJobBackend::new(
            vec![Err(job_not_due_error())],
            Vec::new(),
        ));
        let runtime =
            IndexerRuntime::with_backend(backend.clone(), Metrics::new()?, Duration::from_secs(1));

        runtime.run_job(JobKind::RetentionPurge).await;

        assert_eq!(backend.calls(), vec![BackendCall::Claim("retention_purge")]);
        Ok(())
    }

    #[tokio::test]
    async fn runtime_runs_claimed_jobs() -> anyhow::Result<()> {
        let backend = Arc::new(FakeIndexerJobBackend::new(vec![Ok(())], vec![Ok(())]));
        let runtime =
            IndexerRuntime::with_backend(backend.clone(), Metrics::new()?, Duration::from_secs(1));

        runtime.run_job(JobKind::PolicySnapshotGc).await;

        assert_eq!(
            backend.calls(),
            vec![
                BackendCall::Claim("policy_snapshot_gc"),
                BackendCall::Run(JobKind::PolicySnapshotGc),
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn runtime_run_tick_processes_every_job_and_records_success_metrics() -> anyhow::Result<()>
    {
        let backend = Arc::new(FakeIndexerJobBackend::new(
            std::iter::repeat_with(|| Ok(()))
                .take(JobKind::ALL.len())
                .collect(),
            std::iter::repeat_with(|| Ok(()))
                .take(JobKind::ALL.len())
                .collect(),
        ));
        let metrics = Metrics::new()?;
        let runtime =
            IndexerRuntime::with_backend(backend.clone(), metrics.clone(), Duration::from_secs(1));

        runtime.run_tick().await;

        let mut expected = Vec::with_capacity(JobKind::ALL.len() * 2);
        for job in JobKind::ALL {
            expected.push(BackendCall::Claim(job.job_key()));
            expected.push(BackendCall::Run(job));
        }
        assert_eq!(backend.calls(), expected);

        let rendered = metrics.render()?;
        assert!(rendered.contains("indexer_job_outcomes_total"));
        assert!(
            rendered.contains("operation=\"indexer_runtime.retention_purge\",outcome=\"success\"")
        );
        assert!(rendered.contains(
            "operation=\"indexer_runtime.rss_subscription_backfill\",outcome=\"success\""
        ));
        Ok(())
    }

    #[tokio::test]
    async fn runtime_continues_after_job_failures() -> anyhow::Result<()> {
        let backend = Arc::new(FakeIndexerJobBackend::new(
            vec![Ok(()), Ok(())],
            vec![
                Err(DataError::JobFailed {
                    operation: "job run retention purge",
                    job_key: "retention_purge",
                    error_code: Some("P0001".to_string()),
                    error_detail: Some("job_failed".to_string()),
                }),
                Ok(()),
            ],
        ));
        let runtime =
            IndexerRuntime::with_backend(backend.clone(), Metrics::new()?, Duration::from_secs(1));

        runtime.run_job(JobKind::RetentionPurge).await;
        runtime.run_job(JobKind::RateLimitStatePurge).await;

        assert_eq!(
            backend.calls(),
            vec![
                BackendCall::Claim("retention_purge"),
                BackendCall::Run(JobKind::RetentionPurge),
                BackendCall::Claim("rate_limit_state_purge"),
                BackendCall::Run(JobKind::RateLimitStatePurge),
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn runtime_claim_failures_record_failure_metrics_without_running_jobs()
    -> anyhow::Result<()> {
        let backend = Arc::new(FakeIndexerJobBackend::new(
            vec![Err(DataError::JobFailed {
                operation: "job claim next",
                job_key: "base_score_refresh_recent",
                error_code: Some("XX001".to_string()),
                error_detail: Some("claim_failed".to_string()),
            })],
            Vec::new(),
        ));
        let metrics = Metrics::new()?;
        let runtime =
            IndexerRuntime::with_backend(backend.clone(), metrics.clone(), Duration::from_secs(1));

        runtime.run_job(JobKind::BaseScoreRefreshRecent).await;

        assert_eq!(
            backend.calls(),
            vec![BackendCall::Claim("base_score_refresh_recent")]
        );
        let rendered = metrics.render()?;
        assert!(rendered.contains(
            "operation=\"indexer_runtime.base_score_refresh_recent\",outcome=\"failure\""
        ));
        Ok(())
    }

    #[tokio::test]
    async fn spawned_runtime_starts_ticks_before_abort() -> anyhow::Result<()> {
        let backend = Arc::new(FakeIndexerJobBackend::new(
            std::iter::repeat_with(|| Err(job_not_due_error()))
                .take(JobKind::ALL.len() * 4)
                .collect(),
            Vec::new(),
        ));
        let runtime = IndexerRuntime::with_backend(
            backend.clone(),
            Metrics::new()?,
            Duration::from_millis(1),
        );

        let handle = runtime.spawn();
        timeout(Duration::from_secs(1), async {
            loop {
                if !backend.calls().is_empty() {
                    break;
                }
                yield_now().await;
            }
        })
        .await?;
        handle.abort();
        let _ = handle.await;

        assert!(
            backend
                .calls()
                .iter()
                .any(|call| matches!(call, BackendCall::Claim("retention_purge"))),
            "spawned runtime never attempted the first scheduled job"
        );
        Ok(())
    }
}
