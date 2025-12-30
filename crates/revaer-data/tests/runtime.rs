use std::path::Path;

use revaer_data::RuntimeStore;
use revaer_events::TorrentState;
use revaer_test_support::postgres::start_postgres;
use revaer_torrent_core::{TorrentProgress, TorrentRates, TorrentStatus};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
async fn runtime_store_persists_status_and_fs_jobs() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping runtime_store_persists_status_and_fs_jobs: {err}");
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
        name: Some("demo".to_string()),
        state: TorrentState::Downloading,
        progress: TorrentProgress {
            bytes_downloaded: 1_024,
            bytes_total: 2_048,
            eta_seconds: Some(120),
        },
        rates: TorrentRates {
            download_bps: 10_000,
            upload_bps: 1_000,
            ratio: 0.5,
        },
        files: None,
        library_path: Some("/downloads/demo".to_string()),
        download_dir: Some("/downloads".to_string()),
        comment: Some("hello".to_string()),
        source: Some("source".to_string()),
        private: Some(false),
        sequential: false,
        added_at: chrono::Utc::now(),
        completed_at: None,
        last_updated: chrono::Utc::now(),
    };

    store.upsert_status(&status).await?;
    let mut statuses = store.load_statuses().await?;
    assert_eq!(statuses.len(), 1);
    let persisted = statuses
        .pop()
        .ok_or_else(|| anyhow::anyhow!("persisted status missing"))?;
    assert_eq!(persisted.id, torrent_id);
    assert_eq!(persisted.state, TorrentState::Downloading);

    let source = Path::new("/tmp/source");
    store.mark_fs_job_started(torrent_id, source).await?;
    store
        .mark_fs_job_completed(torrent_id, source, Path::new("/tmp/dest"), Some("copy"))
        .await?;
    let job_state = store
        .fetch_fs_job_state(torrent_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("fs job state missing"))?;
    assert_eq!(job_state.status, "moved");
    assert_eq!(job_state.attempt, 1);
    assert_eq!(job_state.src_path, "/tmp/source");

    store.remove_torrent(torrent_id).await?;
    assert!(store.load_statuses().await?.is_empty());

    let failed_id = Uuid::new_v4();
    store.mark_fs_job_failed(failed_id, "boom").await?;
    let failed_state = store.fetch_fs_job_state(failed_id).await?;
    assert!(
        failed_state.is_none(),
        "failed state is tracked only for known jobs"
    );

    Ok(())
}
