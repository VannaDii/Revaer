use std::net::{IpAddr, TcpListener};
use std::process::Command;
use std::sync::LazyLock;

use anyhow::Result;
use revaer_app::{AppError, run_app, run_app_with_database_url};
use revaer_config::{AppMode, ConfigService, SettingsChangeset, SettingsFacade};
use revaer_test_support::postgres::start_postgres;
use tokio::sync::{Mutex, MutexGuard};
use tokio::time::{Duration, timeout};

static BOOTSTRAP_TEST_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

async fn bootstrap_test_guard() -> MutexGuard<'static, ()> {
    BOOTSTRAP_TEST_MUTEX.lock().await
}

fn run_bootstrap_child(
    test_name: &str,
    envs: &[(&str, &str)],
    removed_envs: &[&str],
) -> Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    command.arg("--exact").arg(test_name).arg("--nocapture");
    for (key, value) in envs {
        command.env(key, value);
    }
    for key in removed_envs {
        command.env_remove(key);
    }

    assert!(command.status()?.success(), "child bootstrap test failed");
    Ok(())
}

#[tokio::test]
async fn run_app_requires_database_url_env() -> Result<()> {
    let _guard = bootstrap_test_guard().await;
    if std::env::var_os("REVAER_BOOTSTRAP_CHILD_MISSING_DATABASE_URL").is_some() {
        let err = timeout(Duration::from_secs(2), run_app())
            .await
            .expect("run_app should fail fast when DATABASE_URL is missing")
            .expect_err("missing DATABASE_URL should fail bootstrap");
        assert!(matches!(
            err,
            AppError::MissingEnv {
                name: "DATABASE_URL"
            }
        ));
        return Ok(());
    }

    run_bootstrap_child(
        "run_app_requires_database_url_env",
        &[("REVAER_BOOTSTRAP_CHILD_MISSING_DATABASE_URL", "1")],
        &[
            "DATABASE_URL",
            "REVAER_SECRET_KEY_ID",
            "REVAER_SECRET_KEY",
            "REVAER_ENABLE_OTEL",
            "REVAER_OTEL_SERVICE_NAME",
            "REVAER_OTEL_EXPORTER",
        ],
    )
}

#[tokio::test]
async fn run_app_rejects_missing_secret_env_pair() -> Result<()> {
    let _guard = bootstrap_test_guard().await;
    if std::env::var_os("REVAER_BOOTSTRAP_CHILD_MISSING_SECRET_ENV").is_some() {
        let err = timeout(Duration::from_secs(2), run_app())
            .await
            .expect("run_app should fail fast on partial secret session env")
            .expect_err("partial secret session env should fail bootstrap");
        assert!(matches!(
            err,
            AppError::MissingEnv {
                name: "REVAER_SECRET_KEY"
            }
        ));
        return Ok(());
    }

    run_bootstrap_child(
        "run_app_rejects_missing_secret_env_pair",
        &[
            ("REVAER_BOOTSTRAP_CHILD_MISSING_SECRET_ENV", "1"),
            ("DATABASE_URL", "postgres://unused"),
            ("REVAER_SECRET_KEY_ID", "key-id"),
        ],
        &["REVAER_SECRET_KEY"],
    )
}

#[tokio::test]
async fn run_app_rejects_empty_secret_env_value() -> Result<()> {
    let _guard = bootstrap_test_guard().await;
    if std::env::var_os("REVAER_BOOTSTRAP_CHILD_EMPTY_SECRET_ENV").is_some() {
        let err = timeout(Duration::from_secs(2), run_app())
            .await
            .expect("run_app should fail fast on empty secret session env")
            .expect_err("empty secret session env should fail bootstrap");
        assert!(matches!(
            err,
            AppError::InvalidConfig {
                field: "REVAER_SECRET_KEY",
                reason: "empty",
                ..
            }
        ));
        return Ok(());
    }

    run_bootstrap_child(
        "run_app_rejects_empty_secret_env_value",
        &[
            ("REVAER_BOOTSTRAP_CHILD_EMPTY_SECRET_ENV", "1"),
            ("DATABASE_URL", "postgres://unused"),
            ("REVAER_SECRET_KEY_ID", "key-id"),
            ("REVAER_SECRET_KEY", "   "),
        ],
        &[],
    )
}

#[tokio::test]
async fn run_app_rejects_empty_secret_key_id_env_value() -> Result<()> {
    let _guard = bootstrap_test_guard().await;
    if std::env::var_os("REVAER_BOOTSTRAP_CHILD_EMPTY_SECRET_KEY_ID").is_some() {
        let err = timeout(Duration::from_secs(2), run_app())
            .await
            .expect("run_app should fail fast on empty secret key id")
            .expect_err("empty secret key id should fail bootstrap");
        assert!(matches!(
            err,
            AppError::InvalidConfig {
                field: "REVAER_SECRET_KEY_ID",
                reason: "empty",
                ..
            }
        ));
        return Ok(());
    }

    run_bootstrap_child(
        "run_app_rejects_empty_secret_key_id_env_value",
        &[
            ("REVAER_BOOTSTRAP_CHILD_EMPTY_SECRET_KEY_ID", "1"),
            ("DATABASE_URL", "postgres://unused"),
            ("REVAER_SECRET_KEY_ID", "   "),
            ("REVAER_SECRET_KEY", "secret-value"),
        ],
        &[],
    )
}

#[tokio::test]
async fn run_app_reads_env_database_url_and_surfaces_bind_failures() -> Result<()> {
    let _guard = bootstrap_test_guard().await;
    if std::env::var_os("REVAER_BOOTSTRAP_CHILD_ENV_BIND_CONFLICT").is_some() {
        let database_url = std::env::var("DATABASE_URL")
            .expect("child env-bind-conflict test requires DATABASE_URL");
        let reserved_listener = TcpListener::bind(("127.0.0.1", 0))?;
        let reserved_port = i32::from(reserved_listener.local_addr()?.port());

        let service = ConfigService::new(database_url.clone()).await?;
        let mut app_profile = service.get_app_profile().await?;
        app_profile.immutable_keys.clear();
        app_profile.mode = AppMode::Setup;
        app_profile.bind_addr = IpAddr::from([127, 0, 0, 1]);
        app_profile.http_port = reserved_port;
        service
            .apply_changeset(
                "tester",
                "bootstrap-env-bind-conflict",
                SettingsChangeset {
                    app_profile: Some(app_profile),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let err = timeout(Duration::from_secs(2), run_app())
            .await
            .expect("run_app should surface bind failure without serving")
            .expect_err("occupied socket should fail api server startup");
        assert!(matches!(err, AppError::ApiServer { .. }), "{err:?}");
        return Ok(());
    }

    let postgres = match start_postgres() {
        Ok(database) => database,
        Err(err) => {
            eprintln!("skipping run_app_reads_env_database_url_and_surfaces_bind_failures: {err}");
            return Ok(());
        }
    };

    run_bootstrap_child(
        "run_app_reads_env_database_url_and_surfaces_bind_failures",
        &[
            ("REVAER_BOOTSTRAP_CHILD_ENV_BIND_CONFLICT", "1"),
            ("DATABASE_URL", postgres.connection_string()),
            ("REVAER_ENABLE_OTEL", "true"),
            ("REVAER_OTEL_SERVICE_NAME", "revaer-app-tests"),
            ("REVAER_OTEL_EXPORTER", "http://127.0.0.1:4318"),
        ],
        &[],
    )
}

#[tokio::test]
async fn run_app_reads_secret_session_env_and_surfaces_bind_failures() -> Result<()> {
    let _guard = bootstrap_test_guard().await;
    if std::env::var_os("REVAER_BOOTSTRAP_CHILD_SECRET_ENV_BIND_CONFLICT").is_some() {
        let database_url = std::env::var("DATABASE_URL")
            .expect("child secret-env bind-conflict test requires DATABASE_URL");
        let reserved_listener = TcpListener::bind(("127.0.0.1", 0))?;
        let reserved_port = i32::from(reserved_listener.local_addr()?.port());

        let service = ConfigService::new(database_url.clone()).await?;
        let mut app_profile = service.get_app_profile().await?;
        app_profile.immutable_keys.clear();
        app_profile.mode = AppMode::Setup;
        app_profile.bind_addr = IpAddr::from([127, 0, 0, 1]);
        app_profile.http_port = reserved_port;
        service
            .apply_changeset(
                "tester",
                "bootstrap-secret-env-bind-conflict",
                SettingsChangeset {
                    app_profile: Some(app_profile),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let err = timeout(Duration::from_secs(2), run_app())
            .await
            .expect("run_app should surface bind failure with secret session env")
            .expect_err("occupied socket should fail api server startup");
        assert!(matches!(err, AppError::ApiServer { .. }), "{err:?}");
        return Ok(());
    }

    let postgres = match start_postgres() {
        Ok(database) => database,
        Err(err) => {
            eprintln!(
                "skipping run_app_reads_secret_session_env_and_surfaces_bind_failures: {err}"
            );
            return Ok(());
        }
    };

    run_bootstrap_child(
        "run_app_reads_secret_session_env_and_surfaces_bind_failures",
        &[
            ("REVAER_BOOTSTRAP_CHILD_SECRET_ENV_BIND_CONFLICT", "1"),
            ("DATABASE_URL", postgres.connection_string()),
            ("REVAER_SECRET_KEY_ID", "  test-key  "),
            ("REVAER_SECRET_KEY", "  test-secret  "),
        ],
        &[],
    )
}

#[tokio::test]
async fn run_app_with_database_url_rejects_public_setup_bind_from_persisted_config() -> Result<()> {
    let _guard = bootstrap_test_guard().await;
    if std::env::var_os("REVAER_BOOTSTRAP_CHILD_PUBLIC_SETUP_BIND").is_some() {
        let database_url = std::env::var("DATABASE_URL")
            .expect("child public-setup-bind test requires DATABASE_URL");
        let service = ConfigService::new(database_url.clone()).await?;
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
            run_app_with_database_url(database_url),
        )
        .await
        .expect("bootstrap should fail fast instead of serving")
        .expect_err("public setup bind should fail validation");
        assert!(
            matches!(
                err,
                AppError::InvalidConfig {
                    field: "bind_addr",
                    reason: "non_loopback_in_setup",
                    ..
                }
            ),
            "unexpected bootstrap error: {err:?}"
        );
        return Ok(());
    }

    let postgres = match start_postgres() {
        Ok(database) => database,
        Err(err) => {
            eprintln!(
                "skipping run_app_with_database_url_rejects_public_setup_bind_from_persisted_config: {err}"
            );
            return Ok(());
        }
    };

    run_bootstrap_child(
        "run_app_with_database_url_rejects_public_setup_bind_from_persisted_config",
        &[
            ("REVAER_BOOTSTRAP_CHILD_PUBLIC_SETUP_BIND", "1"),
            ("DATABASE_URL", postgres.connection_string()),
        ],
        &[],
    )
}

#[tokio::test]
async fn run_app_with_database_url_surfaces_bind_failures_without_child_process() -> Result<()> {
    let _guard = bootstrap_test_guard().await;
    if std::env::var_os("REVAER_BOOTSTRAP_CHILD_DIRECT_BIND_CONFLICT").is_some() {
        let database_url = std::env::var("DATABASE_URL")
            .expect("child direct bind-conflict test requires DATABASE_URL");
        let reserved_listener = TcpListener::bind(("127.0.0.1", 0))?;
        let reserved_port = i32::from(reserved_listener.local_addr()?.port());

        let service = ConfigService::new(database_url.clone()).await?;
        let mut app_profile = service.get_app_profile().await?;
        app_profile.immutable_keys.clear();
        app_profile.mode = AppMode::Setup;
        app_profile.bind_addr = IpAddr::from([127, 0, 0, 1]);
        app_profile.http_port = reserved_port;
        service
            .apply_changeset(
                "tester",
                "bootstrap-bind-conflict-direct",
                SettingsChangeset {
                    app_profile: Some(app_profile),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let err = timeout(
            Duration::from_secs(2),
            run_app_with_database_url(database_url),
        )
        .await
        .expect("bootstrap should surface bind failure without serving")
        .expect_err("occupied socket should fail api server startup");
        assert!(matches!(err, AppError::ApiServer { .. }), "{err:?}");
        return Ok(());
    }

    let postgres = match start_postgres() {
        Ok(database) => database,
        Err(err) => {
            eprintln!(
                "skipping run_app_with_database_url_surfaces_bind_failures_without_child_process: {err}"
            );
            return Ok(());
        }
    };

    run_bootstrap_child(
        "run_app_with_database_url_surfaces_bind_failures_without_child_process",
        &[
            ("REVAER_BOOTSTRAP_CHILD_DIRECT_BIND_CONFLICT", "1"),
            ("DATABASE_URL", postgres.connection_string()),
        ],
        &[],
    )
}

#[tokio::test]
async fn run_app_with_database_url_rejects_zero_http_port_from_persisted_config() -> Result<()> {
    let _guard = bootstrap_test_guard().await;
    let postgres = match start_postgres() {
        Ok(database) => database,
        Err(err) => {
            eprintln!(
                "skipping run_app_with_database_url_rejects_zero_http_port_from_persisted_config: {err}"
            );
            return Ok(());
        }
    };

    let service = ConfigService::new(postgres.connection_string()).await?;
    let mut app_profile = service.get_app_profile().await?;
    app_profile.immutable_keys.clear();
    app_profile.mode = AppMode::Setup;
    app_profile.bind_addr = IpAddr::from([127, 0, 0, 1]);
    app_profile.http_port = 0;
    let err = service
        .apply_changeset(
            "tester",
            "bootstrap-zero-port",
            SettingsChangeset {
                app_profile: Some(app_profile),
                ..SettingsChangeset::default()
            },
        )
        .await
        .expect_err("zero http_port should be rejected before bootstrap starts");
    assert!(matches!(
        err,
        revaer_config::ConfigError::InvalidField {
            field,
            value,
            reason,
            ..
        } if field == "http_port" && value.as_deref() == Some("0") && reason == "must be between 1 and 65535"
    ));
    Ok(())
}

#[tokio::test]
async fn run_app_with_database_url_surfaces_bind_failures_for_valid_persisted_config() -> Result<()>
{
    let _guard = bootstrap_test_guard().await;
    if std::env::var_os("REVAER_BOOTSTRAP_CHILD_BIND_CONFLICT").is_some() {
        let database_url =
            std::env::var("DATABASE_URL").expect("child bind-conflict test requires DATABASE_URL");
        let reserved_listener = TcpListener::bind(("127.0.0.1", 0))?;
        let reserved_port = i32::from(reserved_listener.local_addr()?.port());

        let service = ConfigService::new(database_url.clone()).await?;
        let mut app_profile = service.get_app_profile().await?;
        app_profile.immutable_keys.clear();
        app_profile.mode = AppMode::Setup;
        app_profile.bind_addr = IpAddr::from([127, 0, 0, 1]);
        app_profile.http_port = reserved_port;
        service
            .apply_changeset(
                "tester",
                "bootstrap-bind-conflict",
                SettingsChangeset {
                    app_profile: Some(app_profile),
                    ..SettingsChangeset::default()
                },
            )
            .await?;

        let err = timeout(
            Duration::from_secs(2),
            run_app_with_database_url(database_url),
        )
        .await
        .expect("bootstrap should surface bind failure without serving")
        .expect_err("occupied socket should fail api server startup");
        assert!(matches!(err, AppError::ApiServer { .. }), "{err:?}");
        return Ok(());
    }

    let postgres = match start_postgres() {
        Ok(database) => database,
        Err(err) => {
            eprintln!(
                "skipping run_app_with_database_url_surfaces_bind_failures_for_valid_persisted_config: {err}"
            );
            return Ok(());
        }
    };

    assert!(
        Command::new(std::env::current_exe()?)
            .env("DATABASE_URL", postgres.connection_string())
            .env("REVAER_TEST_DATABASE_URL", postgres.connection_string())
            .env("REVAER_BOOTSTRAP_CHILD_BIND_CONFLICT", "1")
            .arg("--exact")
            .arg("run_app_with_database_url_surfaces_bind_failures_for_valid_persisted_config")
            .arg("--nocapture")
            .status()?
            .success(),
        "child bind-conflict bootstrap test failed"
    );
    Ok(())
}

#[tokio::test]
async fn run_app_with_database_url_rejects_out_of_range_http_port_changes_before_bootstrap()
-> Result<()> {
    let _guard = bootstrap_test_guard().await;
    let postgres = match start_postgres() {
        Ok(database) => database,
        Err(err) => {
            eprintln!(
                "skipping run_app_with_database_url_rejects_out_of_range_http_port_changes_before_bootstrap: {err}"
            );
            return Ok(());
        }
    };

    let service = ConfigService::new(postgres.connection_string()).await?;
    let mut app_profile = service.get_app_profile().await?;
    app_profile.immutable_keys.clear();
    app_profile.mode = AppMode::Setup;
    app_profile.bind_addr = IpAddr::from([127, 0, 0, 1]);
    app_profile.http_port = 70_000;
    let err = service
        .apply_changeset(
            "tester",
            "bootstrap-out-of-range-port",
            SettingsChangeset {
                app_profile: Some(app_profile),
                ..SettingsChangeset::default()
            },
        )
        .await
        .expect_err("out of range http_port should be rejected before bootstrap starts");
    assert!(matches!(
        err,
        revaer_config::ConfigError::InvalidField {
            field,
            value,
            reason,
            ..
        } if field == "http_port" && value.as_deref() == Some("70000") && reason == "must be between 1 and 65535"
    ));
    Ok(())
}
