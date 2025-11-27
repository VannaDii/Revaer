use std::future::Future;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Utc;
use revaer_config::{
    ApiKeyPatch, AppMode, ConfigService, SecretPatch, SettingsChangeset, SettingsFacade,
    SettingsPayload,
};
use revaer_data::config as data_config;
use revaer_test_support::docker;
use serde_json::{Value, json};
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

    if !docker::available() {
        eprintln!("skipping config integration tests: docker socket missing");
        return Ok(());
    }

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
    let ports = container
        .ports()
        .await
        .context("failed to inspect postgres container ports")?;
    let Some(port) = ports
        .map_to_host_port_ipv4(ContainerPort::Tcp(5432))
        .or_else(|| ports.map_to_host_port_ipv6(ContainerPort::Tcp(5432)))
    else {
        eprintln!("skipping config integration tests: failed to resolve postgres host port");
        return Ok(());
    };
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
        let zero_ttl = service
            .issue_setup_token(Duration::from_secs(0), "tester")
            .await;
        assert!(zero_ttl.is_err(), "zero TTL should be rejected");

        let token = service
            .issue_setup_token(Duration::from_secs(60), "tester")
            .await?;
        assert!(
            token.expires_at > Utc::now(),
            "token should be future dated"
        );
        service
            .validate_setup_token(&token.plaintext)
            .await
            .expect("fresh token should validate");

        let wrong = service.consume_setup_token("bogus").await;
        assert!(wrong.is_err(), "expected invalid token failure");

        service
            .consume_setup_token(&token.plaintext)
            .await
            .expect("first consume succeeds");

        let consumed_validation = service.validate_setup_token(&token.plaintext).await;
        assert!(
            consumed_validation.is_err(),
            "consumed token should fail validation"
        );

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

#[tokio::test]
#[serial]
async fn engine_profile_updates_propagate_within_two_seconds() -> Result<()> {
    const DEADLINE: Duration = Duration::from_secs(2);
    with_config_service(|service| async move {
        let (_initial, mut watcher) = service.watch_settings(Duration::from_millis(100)).await?;
        let new_limit = 2_500_000_i64;
        let start = Instant::now();

        service
            .apply_changeset(
                "tester",
                "rate_limit_adjustment",
                SettingsChangeset {
                    engine_profile: Some(json!({ "max_download_bps": new_limit })),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let updated = timeout(DEADLINE, watcher.next()).await??;
        let elapsed = start.elapsed();
        assert!(
            elapsed <= DEADLINE,
            "engine profile update took {elapsed:?}, expected <= {DEADLINE:?}"
        );
        assert_eq!(
            updated.engine_profile.max_download_bps,
            Some(new_limit),
            "max_download_bps did not propagate"
        );

        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn api_key_lifecycle_covers_create_update_delete() -> Result<()> {
    with_config_service(|service| async move {
        service
            .apply_changeset(
                "tester",
                "create_api_key",
                SettingsChangeset {
                    api_keys: vec![ApiKeyPatch::Upsert {
                        key_id: "cli".to_string(),
                        label: Some("CLI client".to_string()),
                        enabled: Some(true),
                        secret: Some("insecure".to_string()),
                        rate_limit: Some(json!({ "burst": 5, "per_seconds": 60 })),
                    }],
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let auth = service
            .authenticate_api_key("cli", "insecure")
            .await?
            .expect("API key should authenticate");
        assert_eq!(auth.key_id, "cli");
        assert_eq!(auth.label.as_deref(), Some("CLI client"));
        let limit = auth.rate_limit.expect("rate limit should exist");
        assert_eq!(limit.burst, 5);
        assert_eq!(limit.replenish_period, Duration::from_secs(60));

        service
            .apply_changeset(
                "tester",
                "disable_api_key",
                SettingsChangeset {
                    api_keys: vec![ApiKeyPatch::Upsert {
                        key_id: "cli".to_string(),
                        label: None,
                        enabled: Some(false),
                        secret: None,
                        rate_limit: None,
                    }],
                    ..SettingsChangeset::default()
                },
            )
            .await?;
        assert!(
            service
                .authenticate_api_key("cli", "insecure")
                .await?
                .is_none(),
            "disabled key must not authenticate"
        );

        service
            .apply_changeset(
                "tester",
                "rotate_api_key",
                SettingsChangeset {
                    api_keys: vec![ApiKeyPatch::Upsert {
                        key_id: "cli".to_string(),
                        label: Some("CLI rotated".to_string()),
                        enabled: Some(true),
                        secret: Some("rotated".to_string()),
                        rate_limit: Some(json!({ "burst": 20, "per_seconds": 30 })),
                    }],
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        assert!(
            service
                .authenticate_api_key("cli", "insecure")
                .await?
                .is_none(),
            "old secret should be invalidated"
        );

        let rotated = service
            .authenticate_api_key("cli", "rotated")
            .await?
            .expect("rotated secret must authenticate");
        assert_eq!(rotated.label.as_deref(), Some("CLI rotated"));
        let rotated_limit = rotated
            .rate_limit
            .expect("updated rate limit should be present");
        assert_eq!(rotated_limit.burst, 20);
        assert_eq!(rotated_limit.replenish_period, Duration::from_secs(30));

        service
            .apply_changeset(
                "tester",
                "delete_api_key",
                SettingsChangeset {
                    api_keys: vec![ApiKeyPatch::Delete {
                        key_id: "cli".to_string(),
                    }],
                    ..SettingsChangeset::default()
                },
            )
            .await?;
        assert!(
            service
                .authenticate_api_key("cli", "rotated")
                .await?
                .is_none(),
            "deleted key should be gone"
        );

        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn secret_patch_flow_hashes_and_deletes_entries() -> Result<()> {
    with_config_service(|service| async move {
        let pool = service.pool().clone();

        service
            .apply_changeset(
                "tester",
                "set_secret",
                SettingsChangeset {
                    secrets: vec![SecretPatch::Set {
                        name: "webhook".to_string(),
                        value: "super-secret-token".to_string(),
                    }],
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let stored = data_config::fetch_secret_by_name(&pool, "webhook")
            .await?
            .expect("secret should be stored");
        let ciphertext = stored.ciphertext;
        assert!(
            ciphertext != b"super-secret-token",
            "ciphertext should differ from plaintext"
        );
        assert!(
            !ciphertext.is_empty(),
            "ciphertext should contain hashed bytes"
        );

        service
            .apply_changeset(
                "tester",
                "delete_secret",
                SettingsChangeset {
                    secrets: vec![SecretPatch::Delete {
                        name: "webhook".to_string(),
                    }],
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let remaining = data_config::fetch_secret_by_name(&pool, "webhook").await?;
        assert!(remaining.is_none(), "secret should be deleted");

        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn app_profile_patch_updates_all_fields() -> Result<()> {
    with_config_service(|service| async move {
        service
            .apply_changeset(
                "tester",
                "app_profile_expansion",
                SettingsChangeset {
                    app_profile: Some(json!({
                        "instance_name": "cluster-a",
                        "mode": "active",
                        "http_port": 9090,
                        "bind_addr": "0.0.0.0",
                        "telemetry": { "format": "json", "level": "debug" },
                        "features": { "search": true },
                        "immutable_keys": ["auth_api_keys", "fs_policy.*"]
                    })),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let snapshot = service.snapshot().await?;
        let app = snapshot.app_profile;
        assert_eq!(app.instance_name, "cluster-a");
        assert_eq!(app.mode, AppMode::Active);
        assert_eq!(app.http_port, 9090);
        assert_eq!(app.bind_addr.to_string(), "0.0.0.0");
        assert_eq!(
            app.telemetry
                .get("format")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            "json"
        );
        assert!(
            app.features
                .get("search")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            "features.search should be enabled"
        );
        let immutable = app
            .immutable_keys
            .as_array()
            .expect("immutable_keys should be array");
        assert!(
            immutable.iter().any(|value| value == "auth_api_keys"),
            "immutable_keys should include auth_api_keys"
        );

        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn engine_profile_patch_handles_optional_fields() -> Result<()> {
    with_config_service(|service| async move {
        service
            .apply_changeset(
                "tester",
                "engine_profile_update",
                SettingsChangeset {
                    engine_profile: Some(json!({
                        "implementation": "stub",
                        "listen_port": 5500,
                        "dht": true,
                        "encryption": "allow_incoming",
                        "max_active": 12,
                        "max_download_bps": 1_500_000_i64,
                        "max_upload_bps": 750_000_i64,
                        "sequential_default": false,
                        "resume_dir": "/var/cache/revaer",
                        "download_root": "/srv/downloads",
                        "tracker": { "announce": ["https://tracker.example"] }
                    })),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let mut engine = service.get_engine_profile().await?;
        assert_eq!(engine.implementation, "stub");
        assert_eq!(engine.listen_port, Some(5500));
        assert!(engine.dht);
        assert_eq!(engine.encryption, "allow_incoming");
        assert_eq!(engine.max_active, Some(12));
        assert_eq!(engine.max_download_bps, Some(1_500_000));
        assert_eq!(engine.max_upload_bps, Some(750_000));
        assert!(!engine.sequential_default);
        assert_eq!(engine.resume_dir, "/var/cache/revaer");
        assert_eq!(engine.download_root, "/srv/downloads");
        assert!(
            engine
                .tracker
                .get("announce")
                .and_then(Value::as_array)
                .is_some(),
            "tracker payload should include announce list"
        );

        service
            .apply_changeset(
                "tester",
                "engine_limit_reset",
                SettingsChangeset {
                    engine_profile: Some(json!({
                        "listen_port": null,
                        "max_active": null,
                        "max_download_bps": null,
                        "max_upload_bps": null
                    })),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        engine = service.get_engine_profile().await?;
        assert_eq!(engine.listen_port, None);
        assert_eq!(engine.max_active, None);
        assert_eq!(engine.max_download_bps, None);
        assert_eq!(engine.max_upload_bps, None);

        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn fs_policy_patch_updates_multiple_sections() -> Result<()> {
    with_config_service(|service| async move {
        service
            .apply_changeset(
                "tester",
                "fs_policy_update",
                SettingsChangeset {
                    fs_policy: Some(json!({
                        "library_root": "/data/new-library",
                        "extract": true,
                        "par2": "verify",
                        "flatten": true,
                        "move_mode": "copy",
                        "cleanup_keep": ["keep/*.nfo"],
                        "cleanup_drop": ["drop/*.tmp"],
                        "chmod_file": "0644",
                        "chmod_dir": "0755",
                        "owner": "svc",
                        "group": "svc",
                        "umask": "0022",
                        "allow_paths": ["/srv/media"]
                    })),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let mut policy = service.get_fs_policy().await?;
        assert_eq!(policy.library_root, "/data/new-library");
        assert!(policy.extract);
        assert_eq!(policy.par2, "verify");
        assert!(policy.flatten);
        assert_eq!(policy.move_mode, "copy");
        assert_eq!(
            policy
                .cleanup_keep
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_str),
            Some("keep/*.nfo")
        );
        assert_eq!(
            policy
                .cleanup_drop
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_str),
            Some("drop/*.tmp")
        );
        assert_eq!(policy.chmod_file.as_deref(), Some("0644"));
        assert_eq!(policy.chmod_dir.as_deref(), Some("0755"));
        assert_eq!(policy.owner.as_deref(), Some("svc"));
        assert_eq!(policy.group.as_deref(), Some("svc"));
        assert_eq!(policy.umask.as_deref(), Some("0022"));
        assert!(
            policy
                .allow_paths
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_str)
                .is_some(),
            "allow_paths should include at least one entry"
        );

        service
            .apply_changeset(
                "tester",
                "fs_policy_clear_optional",
                SettingsChangeset {
                    fs_policy: Some(json!({
                        "chmod_file": null,
                        "chmod_dir": null,
                        "owner": null,
                        "group": null,
                        "umask": null
                    })),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        policy = service.get_fs_policy().await?;
        assert!(policy.chmod_file.is_none());
        assert!(policy.chmod_dir.is_none());
        assert!(policy.owner.is_none());
        assert!(policy.group.is_none());
        assert!(policy.umask.is_none());

        Ok(())
    })
    .await
}
