use std::future::Future;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use revaer_data::runtime::RuntimeStore;
use revaer_events::TorrentState;
use revaer_test_support::docker;
use revaer_torrent_core::{TorrentFile, TorrentProgress, TorrentRates, TorrentStatus};
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{GenericImage, ImageExt};
use tokio::time::sleep;
use uuid::Uuid;

const POSTGRES_IMAGE: &str = "postgres";
const POSTGRES_TAG: &str = "14-alpine";

async fn with_runtime_store<F, Fut>(test: F) -> Result<()>
where
    F: FnOnce(RuntimeStore) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    if !docker::available() {
        eprintln!("skipping runtime store tests: docker socket missing");
        return Ok(());
    }

    let base_image = GenericImage::new(POSTGRES_IMAGE, POSTGRES_TAG)
        .with_exposed_port(ContainerPort::Tcp(5432))
        .with_wait_for(WaitFor::message_on_stdout(
            "database system is ready to accept connections",
        ));

    let request = base_image
        .with_env_var("POSTGRES_PASSWORD", "password")
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_DB", "postgres");

    let container = request
        .start()
        .await
        .context("failed to start postgres container")?;
    let port = container
        .get_host_port_ipv4(ContainerPort::Tcp(5432))
        .await
        .context("failed to resolve postgres host port")?;
    let url = format!("postgres://postgres:password@127.0.0.1:{port}/postgres");

    let pool = {
        let mut attempts = 0;
        loop {
            match PgPoolOptions::new().max_connections(5).connect(&url).await {
                Ok(pool) => break pool,
                Err(err) => {
                    attempts += 1;
                    if attempts >= 10 {
                        return Err(err).context("failed to connect to ephemeral postgres");
                    }
                    sleep(Duration::from_millis(200)).await;
                }
            }
        }
    };

    let store = RuntimeStore::new(pool.clone())
        .await
        .context("failed to initialise runtime store")?;

    let result = test(store.clone()).await;

    pool.close().await;
    drop(container);

    result
}

fn sample_status() -> TorrentStatus {
    TorrentStatus {
        id: Uuid::new_v4(),
        name: Some("sample".to_string()),
        state: TorrentState::Queued,
        progress: TorrentProgress {
            bytes_downloaded: 0,
            bytes_total: 1,
            eta_seconds: Some(0),
        },
        rates: TorrentRates {
            download_bps: 0,
            upload_bps: 0,
            ratio: 0.0,
        },
        files: Some(vec![TorrentFile {
            index: 0,
            path: "sample.file".to_string(),
            size_bytes: 1,
            bytes_completed: 0,
            priority: revaer_torrent_core::FilePriority::Normal,
            selected: true,
        }]),
        library_path: None,
        download_dir: Some("/downloads".to_string()),
        sequential: false,
        added_at: Utc::now(),
        completed_at: None,
        last_updated: Utc::now(),
    }
}

async fn fetch_fs_job_state_row(
    pool: &sqlx::PgPool,
    torrent_id: Uuid,
) -> Result<sqlx::postgres::PgRow, sqlx::Error> {
    sqlx::query(
        "SELECT status, attempt, src_path, dst_path, transfer_mode, last_error \
         FROM revaer_runtime.fs_job_state($1)",
    )
    .bind(torrent_id)
    .fetch_one(pool)
    .await
}

#[tokio::test]
async fn upsert_and_remove_torrent() -> Result<()> {
    with_runtime_store(|store| async move {
        let mut status = sample_status();
        store.upsert_status(&status).await?;

        let mut persisted = store.load_statuses().await?;
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].state, TorrentState::Queued);

        status.state = TorrentState::Completed;
        status.library_path = Some("/library/sample".to_string());
        status.completed_at = Some(Utc::now());
        status.progress.bytes_downloaded = 1;
        status.progress.bytes_total = 1;
        status.rates.download_bps = 128;
        status.rates.upload_bps = 64;
        status.rates.ratio = 1.0;
        status.last_updated = Utc::now();
        store.upsert_status(&status).await?;

        persisted = store.load_statuses().await?;
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].state, TorrentState::Completed);
        assert_eq!(
            persisted[0].library_path.as_deref(),
            Some("/library/sample")
        );

        store.remove_torrent(status.id).await?;
        let final_statuses = store.load_statuses().await?;
        assert!(final_statuses.is_empty());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn fs_job_state_transitions() -> Result<()> {
    with_runtime_store(|store| async move {
        let torrent_id = Uuid::new_v4();
        let mut status = sample_status();
        status.id = torrent_id;
        store.upsert_status(&status).await?;
        store
            .mark_fs_job_started(torrent_id, Path::new("/tmp/source"))
            .await?;

        let row = fetch_fs_job_state_row(store.pool(), torrent_id).await?;

        assert_eq!(row.get::<String, _>("status"), "moving");
        assert_eq!(row.get::<i16, _>("attempt"), 1);
        assert_eq!(row.get::<String, _>("src_path"), "/tmp/source");
        assert!(row.get::<Option<String>, _>("dst_path").is_none());

        store
            .mark_fs_job_completed(
                torrent_id,
                Path::new("/tmp/source"),
                Path::new("/tmp/dest"),
                Some("copy"),
            )
            .await?;

        let row = fetch_fs_job_state_row(store.pool(), torrent_id).await?;

        assert_eq!(row.get::<String, _>("status"), "moved");
        assert_eq!(
            row.get::<Option<String>, _>("dst_path"),
            Some("/tmp/dest".into())
        );
        assert_eq!(
            row.get::<Option<String>, _>("transfer_mode"),
            Some("copy".into())
        );
        assert!(row.get::<Option<String>, _>("last_error").is_none());

        store
            .mark_fs_job_failed(torrent_id, "permission denied")
            .await?;

        let row = fetch_fs_job_state_row(store.pool(), torrent_id).await?;

        assert_eq!(row.get::<String, _>("status"), "failed");
        assert_eq!(row.get::<i16, _>("attempt"), 2);
        assert_eq!(
            row.get::<Option<String>, _>("last_error"),
            Some("permission denied".into())
        );
        Ok(())
    })
    .await
}

#[tokio::test]
async fn fs_job_completion_without_start_persists_row() -> Result<()> {
    with_runtime_store(|store| async move {
        let torrent_id = Uuid::new_v4();
        let mut status = sample_status();
        status.id = torrent_id;
        store.upsert_status(&status).await?;

        store
            .mark_fs_job_completed(
                torrent_id,
                Path::new("/tmp/source"),
                Path::new("/tmp/dest"),
                Some("copy"),
            )
            .await?;

        let row = fetch_fs_job_state_row(store.pool(), torrent_id).await?;

        assert_eq!(row.get::<String, _>("status"), "moved");
        assert_eq!(row.get::<i16, _>("attempt"), 1);
        assert_eq!(row.get::<String, _>("src_path"), "/tmp/source");
        assert_eq!(
            row.get::<Option<String>, _>("dst_path"),
            Some("/tmp/dest".into())
        );
        Ok(())
    })
    .await
}

#[tokio::test]
async fn fs_job_restart_after_completion_preserves_state() -> Result<()> {
    with_runtime_store(|store| async move {
        let torrent_id = Uuid::new_v4();
        let mut status = sample_status();
        status.id = torrent_id;
        store.upsert_status(&status).await?;

        store
            .mark_fs_job_completed(
                torrent_id,
                Path::new("/tmp/original"),
                Path::new("/tmp/destination"),
                Some("copy"),
            )
            .await?;

        store
            .mark_fs_job_started(torrent_id, Path::new("/tmp/restart"))
            .await?;

        let row = fetch_fs_job_state_row(store.pool(), torrent_id).await?;

        assert_eq!(row.get::<String, _>("status"), "moved");
        assert_eq!(row.get::<i16, _>("attempt"), 1);
        assert_eq!(row.get::<String, _>("src_path"), "/tmp/restart");
        Ok(())
    })
    .await
}
