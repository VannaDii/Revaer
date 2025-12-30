use chrono::Weekday;
use revaer_config::{
    ApiKeyPatch, ApiKeyRateLimit, AppMode, ConfigService, SecretPatch, SettingsChangeset,
    SettingsFacade,
    engine_profile::{AltSpeedConfig, AltSpeedSchedule, MAX_RATE_LIMIT_BPS},
};
use revaer_test_support::postgres::start_postgres;
use std::fs;
use std::path::PathBuf;
use tokio::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn config_service_applies_changes_and_tokens() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping config_service_applies_changes_and_tokens: {err}");
            return Ok(());
        }
    };
    let service = ConfigService::new(postgres.connection_string()).await?;
    let (download_root, resume_dir, library_root) = build_temp_paths()?;

    let (snapshot, mut stream) = service.watch_settings(Duration::from_millis(50)).await?;
    let mut app_profile = snapshot.app_profile.clone();
    app_profile.mode = AppMode::Active;
    app_profile.immutable_keys.clear();

    let mut engine_profile = snapshot.engine_profile.clone();
    engine_profile.dht = !snapshot.engine_profile.dht;
    engine_profile.listen_port = snapshot.engine_profile.listen_port;
    engine_profile.sequential_default = snapshot.engine_profile.sequential_default;
    engine_profile.resume_dir = resume_dir;
    engine_profile.download_root = download_root;

    let mut fs_policy = snapshot.fs_policy.clone();
    fs_policy.library_root = library_root.clone();
    fs_policy.flatten = !snapshot.fs_policy.flatten;
    fs_policy.allow_paths = vec![library_root];
    fs_policy.cleanup_keep = vec!["**/*.mkv".to_string()];
    fs_policy.cleanup_drop = Vec::new();
    fs_policy.move_mode = "copy".to_string();

    let changes = SettingsChangeset {
        app_profile: Some(app_profile),
        engine_profile: Some(engine_profile),
        fs_policy: Some(fs_policy),
        api_keys: vec![ApiKeyPatch::Upsert {
            key_id: "ci-key".to_string(),
            label: Some("ci".to_string()),
            enabled: Some(true),
            expires_at: None,
            secret: Some("super-secret".to_string()),
            rate_limit: Some(Some(ApiKeyRateLimit {
                burst: 10,
                replenish_period: Duration::from_secs(60),
            })),
        }],
        secrets: vec![SecretPatch::Set {
            name: "webhook_token".to_string(),
            value: "topsecret".to_string(),
        }],
    };

    let applied = service
        .apply_changeset("tester", "integration", changes)
        .await?;
    assert!(applied.app_profile.is_some());
    assert!(applied.engine_profile.is_some());
    assert!(applied.fs_policy.is_some());

    let issued = service
        .issue_setup_token(Duration::from_secs(60), "tester")
        .await?;
    service.validate_setup_token(&issued.plaintext).await?;
    service.consume_setup_token(&issued.plaintext).await?;

    let refreshed = service.snapshot().await?;
    assert!(refreshed.revision >= snapshot.revision);
    assert_eq!(refreshed.app_profile.mode, AppMode::Active);
    let profile = service.get_app_profile().await?;
    assert_eq!(profile.mode, AppMode::Active);
    let engine = service.get_engine_profile().await?;
    assert_eq!(
        engine.implementation,
        snapshot.engine_profile.implementation
    );
    let fs_policy = service.get_fs_policy().await?;
    assert_eq!(fs_policy.flatten, !snapshot.fs_policy.flatten);
    let auth = service
        .authenticate_api_key("ci-key", "super-secret")
        .await?
        .ok_or_else(|| anyhow::anyhow!("api key did not authenticate"))?;
    assert_eq!(auth.key_id, "ci-key");

    let updated = tokio::time::timeout(Duration::from_secs(10), stream.next()).await??;
    assert!(updated.revision >= applied.revision);
    assert_eq!(updated.app_profile.mode, AppMode::Active);
    Ok(())
}

#[tokio::test]
async fn engine_profile_update_normalizes_alt_speed() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping engine_profile_update_normalizes_alt_speed: {err}");
            return Ok(());
        }
    };
    let service = ConfigService::new(postgres.connection_string()).await?;

    let profile = service.get_engine_profile().await?;
    let mut update = profile.clone();
    update.alt_speed = AltSpeedConfig {
        download_bps: Some(MAX_RATE_LIMIT_BPS + 1),
        upload_bps: Some(-10),
        schedule: Some(AltSpeedSchedule {
            days: vec![Weekday::Wed, Weekday::Mon, Weekday::Mon],
            start_minutes: 60,
            end_minutes: 150,
        }),
    };

    let changes = SettingsChangeset {
        app_profile: None,
        engine_profile: Some(update),
        fs_policy: None,
        api_keys: Vec::new(),
        secrets: Vec::new(),
    };

    service
        .apply_changeset("tester", "normalize-alt-speed", changes)
        .await?;

    let refreshed = service.get_engine_profile().await?;
    assert_eq!(
        refreshed.alt_speed,
        AltSpeedConfig {
            download_bps: Some(MAX_RATE_LIMIT_BPS),
            upload_bps: None,
            schedule: Some(AltSpeedSchedule {
                days: vec![Weekday::Mon, Weekday::Wed],
                start_minutes: 60,
                end_minutes: 150,
            }),
        }
    );
    Ok(())
}

fn build_temp_paths() -> anyhow::Result<(String, String, String)> {
    let mut base = std::env::temp_dir();
    base.push(format!("revaer-config-{}", Uuid::new_v4()));
    let download_root = base.join("downloads");
    let resume_dir = base.join("resume");
    let library_root = base.join("library");
    fs::create_dir_all(&download_root)?;
    fs::create_dir_all(&resume_dir)?;
    fs::create_dir_all(&library_root)?;
    Ok((
        path_to_string(download_root),
        path_to_string(resume_dir),
        path_to_string(library_root),
    ))
}

fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy().to_string()
}
