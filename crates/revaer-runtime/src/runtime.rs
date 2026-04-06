//! Runtime persistence facade wrapping the data-layer store.
//!
//! # Design
//! - Preserve a narrow runtime-oriented boundary for torrent and filesystem-job persistence.
//! - Delegate storage mechanics to `revaer_data` while keeping downstream crates insulated from
//!   the data-layer module layout.

use std::path::Path;

use revaer_data::DataResult;
use revaer_data::runtime::{FsJobState, RuntimeStore as DataRuntimeStore};
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
    pub const fn pool(&self) -> &PgPool {
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
#[path = "runtime/tests.rs"]
mod tests;
