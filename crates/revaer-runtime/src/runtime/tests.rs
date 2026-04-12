use super::RuntimeStore;
use revaer_events::TorrentState;
use revaer_test_support::postgres::TestDatabase;
use revaer_test_support::postgres::start_postgres;
use revaer_torrent_core::{
    FilePriority, TorrentFile, TorrentProgress, TorrentRates, TorrentStatus,
};
use sqlx::postgres::PgPoolOptions;
#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use std::path::Path;
use uuid::Uuid;

async fn test_store() -> anyhow::Result<(TestDatabase, RuntimeStore)> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping runtime store test: {err}");
            return Err(anyhow::anyhow!("runtime store test skipped"));
        }
    };
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(postgres.connection_string())
        .await?;
    let store = RuntimeStore::new(pool).await?;
    Ok((postgres, store))
}

fn sample_status(torrent_id: Uuid, state: TorrentState) -> TorrentStatus {
    TorrentStatus {
        id: torrent_id,
        name: Some("runtime-demo".to_string()),
        state,
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
        files: Some(vec![
            TorrentFile {
                index: 0,
                path: "runtime-demo/file-a.mkv".to_string(),
                size_bytes: 2048,
                bytes_completed: 2048,
                priority: FilePriority::High,
                selected: true,
            },
            TorrentFile {
                index: 1,
                path: "runtime-demo/file-b.nfo".to_string(),
                size_bytes: 128,
                bytes_completed: 64,
                priority: FilePriority::Skip,
                selected: false,
            },
        ]),
        library_path: Some(".server_root/library/runtime-demo".to_string()),
        download_dir: Some(".server_root/downloads".to_string()),
        comment: Some("runtime".to_string()),
        source: Some("integration".to_string()),
        private: Some(false),
        sequential: false,
        added_at: chrono::Utc::now(),
        completed_at: None,
        last_updated: chrono::Utc::now(),
    }
}

#[tokio::test]
async fn runtime_store_round_trips_status_and_fs_jobs() -> anyhow::Result<()> {
    let Ok((postgres, store)) = test_store().await else {
        return Ok(());
    };
    let _keep_db_alive = postgres;

    let torrent_id = Uuid::new_v4();
    let status = sample_status(torrent_id, TorrentState::Downloading);

    store.upsert_status(&status).await?;
    let _connection = store.pool().acquire().await?;
    let statuses = store.load_statuses().await?;
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].id, torrent_id);
    assert_eq!(statuses[0].state, TorrentState::Downloading);
    assert_eq!(statuses[0].files.as_ref().map(Vec::len), Some(2));
    assert_eq!(
        statuses[0]
            .files
            .as_ref()
            .and_then(|files| files.first())
            .map(|file| file.priority),
        Some(FilePriority::High)
    );

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
async fn runtime_store_round_trips_failed_state_and_completed_job_without_transfer_mode()
-> anyhow::Result<()> {
    let Ok((postgres, store)) = test_store().await else {
        return Ok(());
    };
    let _keep_db_alive = postgres;

    let torrent_id = Uuid::new_v4();
    let status = sample_status(
        torrent_id,
        TorrentState::Failed {
            message: "disk full".to_string(),
        },
    );
    store.upsert_status(&status).await?;

    let statuses = store.load_statuses().await?;
    assert_eq!(statuses.len(), 1);
    assert_eq!(
        statuses[0].state,
        TorrentState::Failed {
            message: "disk full".to_string(),
        }
    );

    let source = Path::new(".server_root/source");
    let destination = Path::new(".server_root/final");
    store.mark_fs_job_started(torrent_id, source).await?;
    store
        .mark_fs_job_completed(torrent_id, source, destination, None)
        .await?;

    let job = store
        .fetch_fs_job_state(torrent_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("expected fs job state"))?;
    assert_eq!(job.status, "moved");
    assert_eq!(job.attempt, 1);
    assert_eq!(job.dst_path.as_deref(), Some(".server_root/final"));
    assert_eq!(job.transfer_mode, None);
    assert_eq!(job.last_error, None);

    Ok(())
}

#[tokio::test]
async fn runtime_store_records_failed_job_for_known_torrent() -> anyhow::Result<()> {
    let Ok((postgres, store)) = test_store().await else {
        return Ok(());
    };
    let _keep_db_alive = postgres;

    let torrent_id = Uuid::new_v4();
    store
        .upsert_status(&sample_status(torrent_id, TorrentState::Queued))
        .await?;
    store
        .mark_fs_job_started(torrent_id, Path::new(".server_root/source"))
        .await?;
    store.mark_fs_job_failed(torrent_id, "boom").await?;

    let job = store
        .fetch_fs_job_state(torrent_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("expected failed fs job"))?;
    assert_eq!(job.status, "failed");
    assert_eq!(job.attempt, 2);
    assert_eq!(job.last_error.as_deref(), Some("boom"));

    Ok(())
}

#[tokio::test]
async fn runtime_store_ignores_failed_fs_job_without_known_torrent() -> anyhow::Result<()> {
    let Ok((postgres, store)) = test_store().await else {
        return Ok(());
    };
    let _keep_db_alive = postgres;

    let missing_id = Uuid::new_v4();
    store.mark_fs_job_failed(missing_id, "boom").await?;
    assert!(store.fetch_fs_job_state(missing_id).await?.is_none());

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn runtime_store_rejects_non_utf8_paths() -> anyhow::Result<()> {
    use revaer_data::DataError;
    use std::path::PathBuf;

    let Ok((postgres, store)) = test_store().await else {
        return Ok(());
    };
    let _keep_db_alive = postgres;

    let torrent_id = Uuid::new_v4();
    store
        .upsert_status(&sample_status(torrent_id, TorrentState::Queued))
        .await?;

    let invalid_path = PathBuf::from(OsString::from_vec(vec![0xff]));
    let error = store
        .mark_fs_job_started(torrent_id, &invalid_path)
        .await
        .expect_err("expected invalid utf-8 path error");
    match error {
        DataError::PathNotUtf8 { field, path } => {
            assert_eq!(field, "fs_job_source");
            assert_eq!(path, invalid_path);
        }
        other => panic!("expected PathNotUtf8, got {other:?}"),
    }

    Ok(())
}
