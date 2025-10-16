use std::future::Future;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use revaer_config::{AppMode, ConfigService, SettingsChangeset, SettingsFacade, SettingsPayload};
use serde_json::json;
use serial_test::serial;
use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{GenericImage, ImageExt};
use tokio::time::{sleep, timeout};

async fn with_config_service<F, Fut>(test: F) -> Result<()>
where
    F: FnOnce(ConfigService) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    if let Ok(url) = std::env::var("REVAER_TEST_DATABASE_URL") {
        let service = ConfigService::new(url).await?;
        return test(service).await;
    }

    configure_docker_host();

    let base_image = GenericImage::new("postgres", "14-alpine")
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
        .context("failed to start postgres container for tests")?;
    let port = container
        .get_host_port_ipv4(ContainerPort::Tcp(5432))
        .await
        .context("failed to resolve postgres host port")?;
    let url = format!("postgres://postgres:password@127.0.0.1:{port}/postgres");

    let mut last_err = None;
    let mut service = None;
    for attempt in 0_u32..10 {
        match ConfigService::new(&url).await {
            Ok(svc) => {
                service = Some(svc);
                break;
            }
            Err(err) => {
                last_err = Some(err);
                let backoff_ms = 200u64 * (u64::from(attempt) + 1);
                sleep(Duration::from_millis(backoff_ms)).await;
            }
        }
    }

    let service = service.ok_or_else(|| {
        anyhow::anyhow!(
            "failed to connect to ephemeral postgres: {}",
            last_err
                .as_ref()
                .map_or_else(|| "unknown error".to_string(), |err| format!("{err:#}"),)
        )
    })?;

    let result = test(service.clone()).await;

    service.pool().close().await;
    drop(container);

    result
}

fn configure_docker_host() {
    if std::env::var_os("DOCKER_HOST").is_some() {
        return;
    }

    if Path::new("/var/run/docker.sock").exists() {
        std::env::set_var("DOCKER_HOST", "unix:///var/run/docker.sock");
        return;
    }

    if let Some(home) = std::env::var_os("HOME") {
        let mut path = std::path::PathBuf::from(home);
        path.push(".docker/run/docker.sock");
        if path.exists() {
            let host = format!("unix://{}", path.display());
            std::env::set_var("DOCKER_HOST", host);
        }
    }
}

#[tokio::test]
#[serial]
async fn first_run_seeding_sets_defaults() -> Result<()> {
    with_config_service(|service| async move {
        let app = service.get_app_profile().await?;
        assert_eq!(app.mode, AppMode::Setup);
        assert_eq!(app.instance_name, "revaer");

        let engine = service.get_engine_profile().await?;
        assert_eq!(engine.implementation, "libtorrent");

        let fs = service.get_fs_policy().await?;
        assert_eq!(fs.library_root, "/data/library");
        assert!(fs.allow_paths.as_array().is_some());

        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn apply_changeset_updates_mutables_and_blocks_immutable() -> Result<()> {
    with_config_service(|service| async move {
        let applied = service
            .apply_changeset(
                "tester",
                "update_instance",
                SettingsChangeset {
                    app_profile: Some(json!({"instance_name": "custom"})),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        assert_eq!(
            applied
                .app_profile
                .expect("app profile present")
                .instance_name,
            "custom"
        );

        service
            .apply_changeset(
                "tester",
                "lock_instance",
                SettingsChangeset {
                    app_profile: Some(json!({"immutable_keys": ["app_profile.instance_name"]})),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let err = service
            .apply_changeset(
                "tester",
                "mutate_locked",
                SettingsChangeset {
                    app_profile: Some(json!({"instance_name": "forbidden"})),
                    ..SettingsChangeset::default()
                },
            )
            .await
            .expect_err("immutable mutation should fail");

        assert!(
            format!("{err:#}").contains("immutable field"),
            "unexpected error: {err:?}"
        );

        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn listen_notify_emits_changes() -> Result<()> {
    with_config_service(|service| async move {
        let mut stream = service.subscribe_changes().await?;

        service
            .apply_changeset(
                "tester",
                "update_port",
                SettingsChangeset {
                    app_profile: Some(json!({"http_port": 8080})),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let change = timeout(Duration::from_secs(5), async {
            loop {
                match stream.next().await {
                    Some(Ok(change)) => break change,
                    Some(Err(err)) => panic!("listen error: {err:#}"),
                    None => {}
                }
            }
        })
        .await
        .context("timed out waiting for LISTEN notification")?;

        assert_eq!(change.table, "app_profile");
        assert!(matches!(change.payload, SettingsPayload::AppProfile(_)));

        let snapshot = service.snapshot().await?;
        assert_eq!(snapshot.app_profile.http_port, 8080);

        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn watcher_falls_back_to_polling() -> Result<()> {
    with_config_service(|service| async move {
        let (initial, mut watcher) = service.watch_settings(Duration::from_millis(100)).await?;
        watcher.disable_listen();

        service
            .apply_changeset(
                "tester",
                "update_engine_limits",
                SettingsChangeset {
                    engine_profile: Some(json!({"max_active": 42})),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let updated = timeout(Duration::from_secs(5), watcher.next()).await??;
        assert!(
            updated.revision > initial.revision,
            "revision did not increase"
        );
        assert_eq!(updated.engine_profile.max_active, Some(42));

        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn setup_token_lifecycle_enforces_consumption_rules() -> Result<()> {
    with_config_service(|service| async move {
        let token = service
            .issue_setup_token(Duration::from_secs(60), "tester")
            .await?;

        let wrong = service.consume_setup_token("bogus").await;
        assert!(wrong.is_err(), "expected invalid token failure");

        service
            .consume_setup_token(&token.plaintext)
            .await
            .expect("first consume succeeds");

        let reuse = service.consume_setup_token(&token.plaintext).await;
        assert!(reuse.is_err(), "setup token reuse should fail");

        let short = service
            .issue_setup_token(Duration::from_millis(50), "tester")
            .await?;
        sleep(Duration::from_millis(60)).await;
        let expired = service.consume_setup_token(&short.plaintext).await;
        assert!(expired.is_err(), "expired token should fail");

        let final_token = service
            .issue_setup_token(Duration::from_secs(120), "tester")
            .await?;
        service
            .consume_setup_token(&final_token.plaintext)
            .await
            .expect("final token consumption succeeds");

        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn revision_monotonicity_for_multi_table_changes() -> Result<()> {
    with_config_service(|service| async move {
        let before = service.snapshot().await?.revision;

        service
            .apply_changeset(
                "tester",
                "multi-update",
                SettingsChangeset {
                    app_profile: Some(json!({"instance_name": "rev-check"})),
                    engine_profile: Some(json!({"sequential_default": false})),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let after = service.snapshot().await?.revision;
        assert_eq!(after, before + 1, "revision should bump once");

        Ok(())
    })
    .await
}
