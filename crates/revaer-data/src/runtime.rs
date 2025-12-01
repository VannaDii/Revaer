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
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]

//! Persistence layer for runtime torrent state and filesystem job tracking.

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use revaer_events::TorrentState;
use revaer_torrent_core::{TorrentFile, TorrentStatus};
use serde_json::{Value, json};
use sqlx::{FromRow, PgPool, Row, types::Json};
use uuid::Uuid;

/// Database-backed repository for runtime state.
#[derive(Clone)]
pub struct RuntimeStore {
    pool: PgPool,
}

const UPSERT_TORRENT_CALL: &str = r"
    SELECT revaer_runtime.upsert_torrent(
        _torrent_id => $1,
        _name => $2,
        _state => $3,
        _state_message => $4,
        _progress_bytes_downloaded => $5,
        _progress_bytes_total => $6,
        _progress_eta_seconds => $7,
        _download_bps => $8,
        _upload_bps => $9,
        _ratio => $10,
        _sequential => $11,
        _library_path => $12,
        _download_dir => $13,
        _payload => $14,
        _files => $15,
        _added_at => $16,
        _completed_at => $17,
        _updated_at => $18
    )
";

const DELETE_TORRENT_CALL: &str = r"
    SELECT revaer_runtime.delete_torrent(_torrent_id => $1)
";

const SELECT_TORRENTS_CALL: &str = r"SELECT * FROM revaer_runtime.list_torrents()";

const FS_JOB_STARTED_CALL: &str = r"
    SELECT revaer_runtime.mark_fs_job_started(_torrent_id => $1, _src_path => $2)
";

const FS_JOB_COMPLETED_CALL: &str = r"
    SELECT revaer_runtime.mark_fs_job_completed(
        _torrent_id => $1,
        _src_path => $2,
        _dst_path => $3,
        _transfer_mode => $4
    )
";

const FS_JOB_FAILED_CALL: &str = r"
    SELECT revaer_runtime.mark_fs_job_failed(_torrent_id => $1, _error => $2)
";

const SELECT_FS_JOB_STATE_CALL: &str = r"
    SELECT status,
           attempt,
           src_path,
           dst_path,
           transfer_mode,
           last_error,
           updated_at
    FROM revaer_runtime.fs_job_state(_torrent_id => $1)
";

impl RuntimeStore {
    /// Initialise the runtime store, applying pending migrations.
    ///
    /// # Errors
    ///
    /// Returns an error if migrations fail or the database is unreachable.
    pub async fn new(pool: PgPool) -> Result<Self> {
        let mut migrator = sqlx::migrate!("./migrations");
        migrator.set_ignore_missing(true);
        migrator
            .run(&pool)
            .await
            .context("failed to run runtime migrations")?;
        Ok(Self { pool })
    }

    /// Access the underlying connection pool.
    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Upsert the provided torrent status into the runtime catalog.
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    pub async fn upsert_status(&self, status: &TorrentStatus) -> Result<()> {
        let (state_label, state_message) = serialize_state(&status.state);
        let files = status
            .files
            .as_ref()
            .map(|files| serde_json::to_value(files).context("failed to serialise torrent files"))
            .transpose()?;

        let download_bps = clamp_i64(status.rates.download_bps);
        let upload_bps = clamp_i64(status.rates.upload_bps);
        let bytes_downloaded = clamp_i64(status.progress.bytes_downloaded);
        let bytes_total = clamp_i64(status.progress.bytes_total);
        let eta_seconds = status
            .progress
            .eta_seconds
            .map(|eta| i64::try_from(eta).unwrap_or(i64::MAX));

        let payload = Json(json!({}));
        let files_json = files.map(Json);
        let state_message_ref = state_message.as_deref();
        let name = status.name.as_deref();
        let library_path = status.library_path.as_deref();
        let download_dir = status.download_dir.as_deref();
        sqlx::query(UPSERT_TORRENT_CALL)
            .bind(status.id)
            .bind(name)
            .bind(state_label)
            .bind(state_message_ref)
            .bind(bytes_downloaded)
            .bind(bytes_total)
            .bind(eta_seconds)
            .bind(download_bps)
            .bind(upload_bps)
            .bind(status.rates.ratio)
            .bind(status.sequential)
            .bind(library_path)
            .bind(download_dir)
            .bind(payload)
            .bind(files_json)
            .bind(status.added_at)
            .bind(status.completed_at)
            .bind(status.last_updated)
            .execute(&self.pool)
            .await
            .context("failed to upsert torrent status")?;

        Ok(())
    }

    /// Remove the torrent record from the runtime catalog.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails.
    pub async fn remove_torrent(&self, torrent_id: Uuid) -> Result<()> {
        sqlx::query(DELETE_TORRENT_CALL)
            .bind(torrent_id)
            .execute(&self.pool)
            .await
            .context("failed to remove torrent from runtime catalog")?;

        Ok(())
    }

    /// Load all persisted torrent statuses from the runtime catalog.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails or data cannot be decoded.
    pub async fn load_statuses(&self) -> Result<Vec<TorrentStatus>> {
        let rows = sqlx::query(SELECT_TORRENTS_CALL)
            .fetch_all(&self.pool)
            .await
            .context("failed to load runtime torrent catalog")?;

        let mut statuses = Vec::with_capacity(rows.len());
        for row in rows {
            let state_label: String = row.try_get("state")?;
            let state_message: Option<String> = row.try_get("state_message")?;
            let state = deserialize_state(&state_label, state_message);
            let files = match row.try_get::<Option<Json<Value>>, _>("files")? {
                Some(Json(value)) if !value.is_null() => Some(
                    serde_json::from_value::<Vec<TorrentFile>>(value)
                        .context("failed to decode persisted torrent file list")?,
                ),
                _ => None,
            };

            statuses.push(TorrentStatus {
                id: row.try_get("torrent_id")?,
                name: row.try_get("name")?,
                state,
                progress: revaer_torrent_core::TorrentProgress {
                    bytes_downloaded: u64::try_from(
                        row.try_get::<i64, _>("progress_bytes_downloaded")?,
                    )
                    .unwrap_or_default(),
                    bytes_total: u64::try_from(row.try_get::<i64, _>("progress_bytes_total")?)
                        .unwrap_or_default(),
                    eta_seconds: row
                        .try_get::<Option<i64>, _>("progress_eta_seconds")?
                        .and_then(|eta| u64::try_from(eta).ok()),
                },
                rates: revaer_torrent_core::TorrentRates {
                    download_bps: u64::try_from(row.try_get::<i64, _>("download_bps")?)
                        .unwrap_or_default(),
                    upload_bps: u64::try_from(row.try_get::<i64, _>("upload_bps")?)
                        .unwrap_or_default(),
                    ratio: row.try_get("ratio")?,
                },
                files,
                library_path: row.try_get("library_path")?,
                download_dir: row.try_get("download_dir")?,
                sequential: row.try_get("sequential")?,
                added_at: row.try_get("added_at")?,
                completed_at: row.try_get("completed_at")?,
                last_updated: row.try_get("updated_at")?,
            });
        }

        Ok(statuses)
    }

    /// Record that filesystem processing has started for a torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if the database update fails.
    pub async fn mark_fs_job_started(&self, torrent_id: Uuid, source: &Path) -> Result<()> {
        let source = source
            .to_str()
            .map(std::borrow::ToOwned::to_owned)
            .ok_or_else(|| anyhow!("fs job source path contains invalid UTF-8"))?;

        sqlx::query(FS_JOB_STARTED_CALL)
            .bind(torrent_id)
            .bind(source)
            .execute(&self.pool)
            .await
            .context("failed to record fs job start")?;

        Ok(())
    }

    /// Mark the filesystem processing job as completed.
    ///
    /// # Errors
    ///
    /// Returns an error if the database update fails.
    pub async fn mark_fs_job_completed(
        &self,
        torrent_id: Uuid,
        source: &Path,
        destination: &Path,
        transfer_mode: Option<&str>,
    ) -> Result<()> {
        let source = source
            .to_str()
            .map(std::borrow::ToOwned::to_owned)
            .ok_or_else(|| anyhow!("fs job source path contains invalid UTF-8"))?;
        let destination = destination
            .to_str()
            .map(std::borrow::ToOwned::to_owned)
            .ok_or_else(|| anyhow!("fs job destination path contains invalid UTF-8"))?;

        sqlx::query(FS_JOB_COMPLETED_CALL)
            .bind(torrent_id)
            .bind(source)
            .bind(destination)
            .bind(transfer_mode)
            .execute(&self.pool)
            .await
            .context("failed to record fs job completion")?;

        Ok(())
    }

    /// Fetch the filesystem job state for a torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub async fn fetch_fs_job_state(&self, torrent_id: Uuid) -> Result<Option<FsJobState>> {
        let row = sqlx::query_as::<_, FsJobStateRow>(SELECT_FS_JOB_STATE_CALL)
            .bind(torrent_id)
            .fetch_optional(&self.pool)
            .await
            .context("failed to load fs job state")?;
        Ok(row.map(FsJobState::from))
    }

    /// Record that filesystem processing failed and capture the error message.
    ///
    /// # Errors
    ///
    /// Returns an error if the database update fails.
    pub async fn mark_fs_job_failed(&self, torrent_id: Uuid, error: &str) -> Result<()> {
        sqlx::query(FS_JOB_FAILED_CALL)
            .bind(torrent_id)
            .bind(error)
            .execute(&self.pool)
            .await
            .context("failed to record fs job failure")?;

        Ok(())
    }
}

fn serialize_state(state: &TorrentState) -> (&'static str, Option<String>) {
    match state {
        TorrentState::Queued => ("queued", None),
        TorrentState::FetchingMetadata => ("fetching_metadata", None),
        TorrentState::Downloading => ("downloading", None),
        TorrentState::Seeding => ("seeding", None),
        TorrentState::Completed => ("completed", None),
        TorrentState::Failed { message } => ("failed", Some(message.clone())),
        TorrentState::Stopped => ("stopped", None),
    }
}

fn deserialize_state(label: &str, message: Option<String>) -> TorrentState {
    match label {
        "queued" => TorrentState::Queued,
        "fetching_metadata" => TorrentState::FetchingMetadata,
        "downloading" => TorrentState::Downloading,
        "seeding" => TorrentState::Seeding,
        "completed" => TorrentState::Completed,
        "failed" => TorrentState::Failed {
            message: message.unwrap_or_else(|| "unknown failure".to_string()),
        },
        "stopped" => TorrentState::Stopped,
        other => {
            tracing::warn!(state = %other, "unknown torrent state encountered in runtime store");
            TorrentState::Stopped
        }
    }
}

fn clamp_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_state_serialisation() {
        let variants = [
            TorrentState::Queued,
            TorrentState::FetchingMetadata,
            TorrentState::Downloading,
            TorrentState::Seeding,
            TorrentState::Completed,
            TorrentState::Stopped,
            TorrentState::Failed {
                message: "failure".to_string(),
            },
        ];

        for state in variants {
            let (label, message) = serialize_state(&state);
            let restored = deserialize_state(label, message);
            match (&state, &restored) {
                (
                    TorrentState::Failed { message: original },
                    TorrentState::Failed { message: round },
                ) => assert_eq!(original, round),
                _ => assert_eq!(format!("{state:?}"), format!("{restored:?}")),
            }
        }
    }

    #[test]
    fn clamp_handles_large_values() {
        assert_eq!(clamp_i64(42), 42);
        assert_eq!(clamp_i64(i64::MAX as u64), i64::MAX);
        assert_eq!(clamp_i64(u64::MAX), i64::MAX);
    }
}
/// Snapshot of filesystem processing state for a torrent.
#[derive(Debug, Clone)]
pub struct FsJobState {
    /// Current status label (e.g., `moving`, `moved`).
    pub status: String,
    /// Number of attempts recorded for the job.
    pub attempt: i16,
    /// Source path tracked by the job.
    pub src_path: String,
    /// Destination path recorded after completion.
    pub dst_path: Option<String>,
    /// Transfer mode used (e.g., copy, move).
    pub transfer_mode: Option<String>,
    /// Last error message, if any.
    pub last_error: Option<String>,
    /// Timestamp of the last update.
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
struct FsJobStateRow {
    status: String,
    attempt: i16,
    src_path: String,
    dst_path: Option<String>,
    transfer_mode: Option<String>,
    last_error: Option<String>,
    updated_at: DateTime<Utc>,
}

impl From<FsJobStateRow> for FsJobState {
    fn from(row: FsJobStateRow) -> Self {
        Self {
            status: row.status,
            attempt: row.attempt,
            src_path: row.src_path,
            dst_path: row.dst_path,
            transfer_mode: row.transfer_mode,
            last_error: row.last_error,
            updated_at: row.updated_at,
        }
    }
}
