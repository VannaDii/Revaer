use std::path::Path;

use revaer_data::RuntimeStore;
use revaer_events::TorrentState;
use revaer_test_support::postgres::start_postgres;
use revaer_torrent_core::{
    FilePriority, TorrentFile, TorrentProgress, TorrentRates, TorrentStatus,
};
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
        library_path: Some(".server_root/downloads/demo".to_string()),
        download_dir: Some(".server_root/downloads".to_string()),
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

    let source = Path::new(".server_root/source");
    store.mark_fs_job_started(torrent_id, source).await?;
    store
        .mark_fs_job_completed(
            torrent_id,
            source,
            Path::new(".server_root/dest"),
            Some("copy"),
        )
        .await?;
    let job_state = store
        .fetch_fs_job_state(torrent_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("fs job state missing"))?;
    assert_eq!(job_state.status, "moved");
    assert_eq!(job_state.attempt, 1);
    assert_eq!(job_state.src_path, ".server_root/source");

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

#[tokio::test]
async fn runtime_store_round_trips_files_and_failed_state() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping runtime_store_round_trips_files_and_failed_state: {err}");
            return Ok(());
        }
    };
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(postgres.connection_string())
        .await?;
    let store = RuntimeStore::new(pool).await?;

    let torrent_id = Uuid::new_v4();
    let added_at = chrono::Utc::now();
    let completed_at = chrono::Utc::now();
    let last_updated = chrono::Utc::now();
    let status = TorrentStatus {
        id: torrent_id,
        name: Some("failed-demo".to_string()),
        state: TorrentState::Failed {
            message: "checksum mismatch".to_string(),
        },
        progress: TorrentProgress {
            bytes_downloaded: 4_096,
            bytes_total: 8_192,
            eta_seconds: Some(42),
        },
        rates: TorrentRates {
            download_bps: 20_000,
            upload_bps: 2_000,
            ratio: 1.25,
        },
        files: Some(vec![
            TorrentFile {
                index: 0,
                path: "disc1/movie.mkv".to_string(),
                size_bytes: 6_000,
                bytes_completed: 4_000,
                priority: FilePriority::High,
                selected: true,
            },
            TorrentFile {
                index: 1,
                path: "disc1/extras.txt".to_string(),
                size_bytes: 2_192,
                bytes_completed: 96,
                priority: FilePriority::Skip,
                selected: false,
            },
        ]),
        library_path: Some(".server_root/library/failed-demo".to_string()),
        download_dir: Some(".server_root/downloads/failed-demo".to_string()),
        comment: Some("comment".to_string()),
        source: Some("source".to_string()),
        private: Some(true),
        sequential: true,
        added_at,
        completed_at: Some(completed_at),
        last_updated,
    };

    store.upsert_status(&status).await?;
    let statuses = store.load_statuses().await?;
    assert_eq!(statuses.len(), 1);
    let persisted = &statuses[0];
    assert_eq!(persisted.id, torrent_id);
    assert_eq!(persisted.name.as_deref(), Some("failed-demo"));
    assert_eq!(
        persisted.state,
        TorrentState::Failed {
            message: "checksum mismatch".to_string(),
        }
    );
    assert_eq!(persisted.progress.bytes_downloaded, 4_096);
    assert_eq!(persisted.progress.bytes_total, 8_192);
    assert_eq!(persisted.progress.eta_seconds, Some(42));
    assert_eq!(persisted.rates.download_bps, 20_000);
    assert_eq!(persisted.rates.upload_bps, 2_000);
    assert_eq!(persisted.rates.ratio, 1.25);
    assert_eq!(persisted.comment.as_deref(), Some("comment"));
    assert_eq!(persisted.source.as_deref(), Some("source"));
    assert_eq!(persisted.private, Some(true));
    assert!(persisted.sequential);
    assert_eq!(persisted.added_at, added_at);
    assert_eq!(persisted.completed_at, Some(completed_at));
    assert_eq!(persisted.last_updated, last_updated);
    let files = persisted
        .files
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("expected persisted file list"))?;
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path, "disc1/movie.mkv");
    assert_eq!(files[0].priority, FilePriority::High);
    assert_eq!(files[1].path, "disc1/extras.txt");
    assert_eq!(files[1].priority, FilePriority::Skip);
    assert!(!files[1].selected);

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn runtime_store_rejects_non_utf8_fs_job_paths() -> anyhow::Result<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping runtime_store_rejects_non_utf8_fs_job_paths: {err}");
            return Ok(());
        }
    };
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(postgres.connection_string())
        .await?;
    let store = RuntimeStore::new(pool).await?;
    let torrent_id = Uuid::new_v4();
    let invalid_path = std::path::PathBuf::from(OsString::from_vec(vec![0x66, 0x6f, 0x80]));

    let start_err = store
        .mark_fs_job_started(torrent_id, &invalid_path)
        .await
        .expect_err("invalid source path should be rejected");
    assert!(matches!(
        start_err,
        revaer_data::DataError::PathNotUtf8 {
            field: "fs_job_source",
            ..
        }
    ));

    let completed_err = store
        .mark_fs_job_completed(
            torrent_id,
            Path::new(".server_root/source"),
            &invalid_path,
            None,
        )
        .await
        .expect_err("invalid destination path should be rejected");
    assert!(matches!(
        completed_err,
        revaer_data::DataError::PathNotUtf8 {
            field: "fs_job_destination",
            ..
        }
    ));

    Ok(())
}
