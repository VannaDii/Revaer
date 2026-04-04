use std::net::IpAddr;

use anyhow::Result;
use revaer_app::{AppError, run_app_with_database_url};
use revaer_config::{AppMode, ConfigService, SettingsChangeset, SettingsFacade};
use revaer_test_support::postgres::start_postgres;
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn run_app_with_database_url_rejects_public_setup_bind_from_persisted_config() -> Result<()> {
    let postgres = match start_postgres() {
        Ok(database) => database,
        Err(err) => {
            eprintln!(
                "skipping run_app_with_database_url_rejects_public_setup_bind_from_persisted_config: {err}"
            );
            return Ok(());
        }
    };

    let service = ConfigService::new(postgres.connection_string()).await?;
    let mut app_profile = service.get_app_profile().await?;
    app_profile.immutable_keys.clear();
    app_profile.mode = AppMode::Setup;
    app_profile.bind_addr = IpAddr::from([192, 168, 10, 20]);
    service
        .apply_changeset(
            "tester",
            "bootstrap-public-bind",
            SettingsChangeset {
                app_profile: Some(app_profile),
                ..SettingsChangeset::default()
            },
        )
        .await?;
    let stored = service.get_app_profile().await?;
    assert_eq!(stored.mode, AppMode::Setup);
    assert_eq!(stored.bind_addr, IpAddr::from([192, 168, 10, 20]));

    let err = timeout(
        Duration::from_secs(2),
        run_app_with_database_url(postgres.connection_string().to_string()),
    )
    .await
    .expect("bootstrap should fail fast instead of serving")
    .expect_err("public setup bind should fail validation");
    assert!(matches!(
        err,
        AppError::InvalidConfig {
            field: "bind_addr",
            reason: "non_loopback_in_setup",
            ..
        }
    ));
    Ok(())
}
