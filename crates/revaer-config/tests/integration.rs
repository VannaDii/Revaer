use revaer_config::{
    ApiKeyPatch, AppMode, ConfigService, SecretPatch, SettingsChangeset, SettingsFacade,
};
use revaer_test_support::postgres::start_postgres;
use serde_json::json;
use tokio::time::Duration;

#[tokio::test]
async fn config_service_applies_changes_and_tokens() -> anyhow::Result<()> {
    let postgres = start_postgres()?;
    let service = ConfigService::new(postgres.connection_string()).await?;

    let (snapshot, mut stream) = service.watch_settings(Duration::from_millis(50)).await?;
    let changes = SettingsChangeset {
        app_profile: Some(json!({ "mode": "active", "immutable_keys": [] })),
        engine_profile: Some(json!({
            "dht": !snapshot.engine_profile.dht,
            "listen_port": snapshot.engine_profile.listen_port,
            "sequential_default": snapshot.engine_profile.sequential_default,
            "resume_dir": snapshot.engine_profile.resume_dir,
            "download_root": snapshot.engine_profile.download_root,
            "tracker": {},
        })),
        fs_policy: Some(json!({
            "flatten": !snapshot.fs_policy.flatten,
            "allow_paths": [snapshot.fs_policy.library_root],
            "cleanup_keep": ["**/*.mkv"],
            "cleanup_drop": [],
            "move_mode": "copy"
        })),
        api_keys: vec![ApiKeyPatch::Upsert {
            key_id: "ci-key".to_string(),
            label: Some("ci".to_string()),
            enabled: Some(true),
            secret: Some("super-secret".to_string()),
            rate_limit: Some(json!({"burst": 10, "per_seconds": 60})),
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
    service
        .validate_setup_token(&issued.plaintext)
        .await
        .expect("setup token should validate before consumption");
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
        .expect("api key should authenticate");
    assert_eq!(auth.key_id, "ci-key");

    let updated = tokio::time::timeout(Duration::from_secs(10), stream.next())
        .await
        .expect("settings watcher should produce a snapshot")?;
    assert!(updated.revision >= applied.revision);
    assert_eq!(updated.app_profile.mode, AppMode::Active);
    Ok(())
}
