//! Runtime persistence facade wrapping the data-layer store.
//!
//! # Design
//! - Preserve a narrow runtime-oriented boundary for torrent and filesystem-job persistence.
//! - Delegate storage mechanics to `revaer_data` while keeping downstream crates insulated from
//!   the data-layer module layout.

use std::path::Path;

use revaer_data::runtime::{FsJobState, RuntimeStore as DataRuntimeStore};
use revaer_data::DataResult;
use revaer_torrent_core::TorrentStatus;
use sqlx::PgPool;
use uuid::Uuid;

/// Runtime persistence facade for torrent status and filesystem job state.
#[derive(Clone)]
pub struct RuntimeStore {
    inner: DataRuntimeStore,
}

impl RuntimeStore {
    /// Initialise the runtime store, applying any pending runtime migrations.
    ///
    /// # Errors
    ///
    /// Returns an error if migrations fail or the database cannot be reached.
    pub async fn new(pool: PgPool) -> DataResult<Self> {
        Ok(Self {
            inner: DataRuntimeStore::new(pool).await?,
        })
    }

    /// Access the underlying connection pool used by the facade.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        self.inner.pool()
    }

    /// Persist or update torrent runtime state.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying database operation fails.
    pub async fn upsert_status(&self, status: &TorrentStatus) -> DataResult<()> {
        self.inner.upsert_status(status).await
    }

    /// Remove a torrent from the runtime catalog.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails.
    pub async fn remove_torrent(&self, torrent_id: Uuid) -> DataResult<()> {
        self.inner.remove_torrent(torrent_id).await
    }

    /// Load all persisted torrent statuses.
    ///
    /// # Errors
    ///
    /// Returns an error if the catalog query fails.
    pub async fn load_statuses(&self) -> DataResult<Vec<TorrentStatus>> {
        self.inner.load_statuses().await
    }

    /// Record the start of a filesystem post-processing job.
    ///
    /// # Errors
    ///
    /// Returns an error if the runtime store cannot persist the start event.
    pub async fn mark_fs_job_started(&self, torrent_id: Uuid, source: &Path) -> DataResult<()> {
        self.inner.mark_fs_job_started(torrent_id, source).await
    }

    /// Record completion of a filesystem post-processing job.
    ///
    /// # Errors
    ///
    /// Returns an error if the completion record cannot be persisted.
    pub async fn mark_fs_job_completed(
        &self,
        torrent_id: Uuid,
        source: &Path,
        destination: &Path,
        transfer_mode: Option<&str>,
    ) -> DataResult<()> {
        self.inner
            .mark_fs_job_completed(torrent_id, source, destination, transfer_mode)
            .await
    }

    /// Fetch the latest filesystem post-processing job state for a torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if the runtime store query fails.
    pub async fn fetch_fs_job_state(&self, torrent_id: Uuid) -> DataResult<Option<FsJobState>> {
        self.inner.fetch_fs_job_state(torrent_id).await
    }

    /// Record a failed filesystem post-processing job.
    ///
    /// # Errors
    ///
    /// Returns an error if the runtime store cannot persist the failure.
    pub async fn mark_fs_job_failed(&self, torrent_id: Uuid, error: &str) -> DataResult<()> {
        self.inner.mark_fs_job_failed(torrent_id, error).await
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeStore;
    use revaer_events::TorrentState;
    use revaer_test_support::postgres::start_postgres;
    use revaer_torrent_core::{TorrentProgress, TorrentRates, TorrentStatus};
    use sqlx::postgres::PgPoolOptions;
    use std::path::Path;
    use uuid::Uuid;

    #[tokio::test]
    async fn runtime_store_round_trips_status_and_fs_jobs() -> anyhow::Result<()> {
        let postgres = match start_postgres() {
            Ok(db) => db,
            Err(err) => {
                eprintln!("skipping runtime_store_round_trips_status_and_fs_jobs: {err}");
                return Ok(());
            }
        };
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(postgres.connection_string())
            .await?;
        let store = RuntimeStore::new(pool.clone()).await?;

        let torrent_id = Uuid::new_v4();
        let status = TorrentStatus {
            id: torrent_id,
            name: Some("runtime-demo".to_string()),
            state: TorrentState::Downloading,
            progress: TorrentProgress {
                bytes_downloaded: 1024,
                bytes_total: 4096,
                eta_seconds: Some(60),
            },
            rates: TorrentRates {
                download_bps: 10_000,
                upload_bps: 1_000,
                ratio: 0.25,
            },
            files: None,
            library_path: Some(".server_root/library/runtime-demo".to_string()),
            download_dir: Some(".server_root/downloads".to_string()),
            comment: Some("runtime".to_string()),
            source: Some("integration".to_string()),
            private: Some(false),
            sequential: false,
            added_at: chrono::Utc::now(),
            completed_at: None,
            last_updated: chrono::Utc::now(),
        };

        store.upsert_status(&status).await?;
        let statuses = store.load_statuses().await?;
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].id, torrent_id);
        assert_eq!(statuses[0].state, TorrentState::Downloading);

        let source = Path::new(".server_root/source");
        let destination = Path::new(".server_root/destination");
        store.mark_fs_job_started(torrent_id, source).await?;
        store
            .mark_fs_job_completed(torrent_id, source, destination, Some("copy"))
            .await?;

        let fs_job = store
            .fetch_fs_job_state(torrent_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("expected persisted fs job state"))?;
        assert_eq!(fs_job.status, "moved");
        assert_eq!(fs_job.attempt, 1);
        assert_eq!(fs_job.src_path, ".server_root/source");
        assert_eq!(fs_job.dst_path.as_deref(), Some(".server_root/destination"));
        assert_eq!(fs_job.transfer_mode.as_deref(), Some("copy"));

        store.remove_torrent(torrent_id).await?;
        assert!(store.load_statuses().await?.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn runtime_store_ignores_failed_fs_job_without_known_torrent() -> anyhow::Result<()> {
        let postgres = match start_postgres() {
            Ok(db) => db,
            Err(err) => {
                eprintln!(
                    "skipping runtime_store_ignores_failed_fs_job_without_known_torrent: {err}"
                );
                return Ok(());
            }
        };
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(postgres.connection_string())
            .await?;
        let store = RuntimeStore::new(pool).await?;

        let missing_id = Uuid::new_v4();
        store.mark_fs_job_failed(missing_id, "boom").await?;
        assert!(store.fetch_fs_job_state(missing_id).await?.is_none());

        Ok(())
    }
}
