use anyhow::anyhow;
use revaer_api::models::{SetupCompleteResponse, SetupStartRequest, SetupStartResponse};
use revaer_config::{ApiKeyPatch, AppMode, ConfigSnapshot, SecretPatch, SettingsChangeset};
use std::io::{self, IsTerminal};
use std::net::IpAddr;
use std::path::Path;

use crate::cli::{SetupCompleteArgs, SetupStartArgs};
use crate::client::{
    AppContext, CliError, CliResult, HEADER_SETUP_TOKEN, classify_problem, random_string,
};

pub(crate) async fn handle_setup_start(ctx: &AppContext, args: SetupStartArgs) -> CliResult<()> {
    let url = ctx
        .base_url
        .join("/admin/setup/start")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let mut request = ctx.client.post(url);

    if args.issued_by.is_some() || args.ttl_seconds.is_some() {
        let payload = SetupStartRequest {
            issued_by: args.issued_by,
            ttl_seconds: args.ttl_seconds,
        };
        request = request.json(&payload);
    }

    let response = request
        .send()
        .await
        .map_err(|err| CliError::failure(anyhow!("request to /admin/setup/start failed: {err}")))?;

    if response.status().is_success() {
        let body = response.json::<SetupStartResponse>().await.map_err(|err| {
            CliError::failure(anyhow!("failed to parse setup start response: {err}"))
        })?;
        println!("{}", body.token);
        println!("expires_at: {}", body.expires_at);
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_setup_complete(
    ctx: &AppContext,
    args: SetupCompleteArgs,
) -> CliResult<()> {
    let token = args
        .token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CliError::validation("setup token is required (flag --token or REVAER_SETUP_TOKEN)")
        })?;

    let bind_addr: IpAddr = args
        .bind
        .parse()
        .map_err(|_| CliError::validation("bind address must be a valid IP address"))?;

    if !bind_addr.is_loopback() {
        return Err(CliError::validation(
            "setup mode must bind to a loopback address",
        ));
    }

    if args.port == 0 {
        return Err(CliError::validation("port must be between 1 and 65535"));
    }

    let passphrase = resolve_passphrase(&args)?;

    let resume_dir = path_to_string(&args.resume_dir)?;
    let download_root = path_to_string(&args.download_root)?;
    let library_root = path_to_string(&args.library_root)?;

    let snapshot = fetch_well_known_snapshot(ctx).await?;

    let api_key_id = args.api_key_id.clone().unwrap_or_else(|| random_string(24));
    let api_key_secret = random_string(48);

    let mut app_profile = snapshot.app_profile;
    app_profile.instance_name = args.instance;
    app_profile.bind_addr = bind_addr;
    app_profile.http_port = i32::from(args.port);
    app_profile.mode = AppMode::Active;

    let mut engine_profile = snapshot.engine_profile;
    engine_profile.implementation = "libtorrent".to_string();
    engine_profile.resume_dir.clone_from(&resume_dir);
    engine_profile.download_root.clone_from(&download_root);

    let fs_policy = build_fs_policy_patch(
        snapshot.fs_policy,
        &library_root,
        &download_root,
        &resume_dir,
    );

    let changeset = SettingsChangeset {
        app_profile: Some(app_profile),
        engine_profile: Some(engine_profile),
        fs_policy: Some(fs_policy),
        api_keys: vec![ApiKeyPatch::Upsert {
            key_id: api_key_id.clone(),
            label: Some(args.api_key_label.clone()),
            enabled: Some(true),
            expires_at: None,
            secret: Some(api_key_secret.clone()),
            rate_limit: None,
        }],
        secrets: vec![SecretPatch::Set {
            name: "encryption_passphrase".to_string(),
            value: passphrase,
        }],
    };

    let url = ctx
        .base_url
        .join("/admin/setup/complete")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_SETUP_TOKEN, token)
        .json(&changeset)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!("request to /admin/setup/complete failed: {err}"))
        })?;

    if response.status().is_success() {
        let body = response
            .json::<SetupCompleteResponse>()
            .await
            .map_err(|err| {
                CliError::failure(anyhow!("failed to parse setup completion response: {err}"))
            })?;
        let instance_name = &body.snapshot.app_profile.instance_name;
        println!("Setup complete for instance '{instance_name}'.");
        if let Some(api_key) = body.api_key {
            println!("API key created (store securely): {api_key}");
        } else {
            println!("No API key issued (auth disabled).");
        }
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

async fn fetch_well_known_snapshot(ctx: &AppContext) -> CliResult<ConfigSnapshot> {
    let url = ctx
        .base_url
        .join("/.well-known/revaer.json")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx.client.get(url).send().await.map_err(|err| {
        CliError::failure(anyhow!("request to /.well-known/revaer.json failed: {err}"))
    })?;

    if response.status().is_success() {
        response
            .json::<ConfigSnapshot>()
            .await
            .map_err(|err| CliError::failure(anyhow!("failed to parse well-known snapshot: {err}")))
    } else {
        Err(classify_problem(response).await)
    }
}

fn path_to_string(path: &Path) -> CliResult<String> {
    path.to_str().map(str::to_string).ok_or_else(|| {
        CliError::validation(format!("path '{}' is not valid UTF-8", path.display()))
    })
}

pub(crate) fn build_fs_policy_patch(
    mut policy: revaer_config::FsPolicy,
    library_root: &str,
    download_root: &str,
    resume_dir: &str,
) -> revaer_config::FsPolicy {
    let mut allow_paths = vec![download_root.to_string(), library_root.to_string()];
    if !allow_paths.iter().any(|p| p == resume_dir) {
        allow_paths.push(resume_dir.to_string());
    }

    policy.library_root = library_root.to_string();
    policy.allow_paths = allow_paths;
    policy
}

pub(crate) fn resolve_passphrase(args: &SetupCompleteArgs) -> CliResult<String> {
    if let Some(value) = &args.passphrase {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(CliError::validation("passphrase cannot be empty"));
        }
        return Ok(trimmed.to_string());
    }

    if io::stdin().is_terminal() {
        let pass = rpassword::prompt_password("Encryption passphrase: ").map_err(|err| {
            CliError::failure(anyhow!("failed to read passphrase from stdin: {err}"))
        })?;
        let trimmed = pass.trim();
        if trimmed.is_empty() {
            return Err(CliError::validation("passphrase cannot be empty"));
        }
        Ok(trimmed.to_string())
    } else {
        Err(CliError::validation(
            "passphrase required; supply via --passphrase when running non-interactively",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Result, anyhow};
    use chrono::Utc;
    use httpmock::prelude::*;
    use reqwest::Client;
    use revaer_config::{
        AppMode, AppProfile, EngineProfile, FsPolicy, TelemetryConfig,
        engine_profile::{AltSpeedConfig, IpFilterConfig, PeerClassesConfig, TrackerConfig},
        normalize_engine_profile,
    };
    use serde_json::json;
    use std::path::PathBuf;
    use tokio::time::{Duration, timeout};
    use uuid::Uuid;

    use crate::client::ApiKeyCredential;

    fn context_with(server: &MockServer, api_key: Option<ApiKeyCredential>) -> Result<AppContext> {
        Ok(AppContext {
            client: Client::new(),
            base_url: server
                .base_url()
                .parse()
                .map_err(|_| anyhow!("valid URL"))?,
            api_key,
        })
    }

    fn context_with_key(server: &MockServer) -> Result<AppContext> {
        context_with(
            server,
            Some(ApiKeyCredential {
                key_id: "key".to_string(),
                secret: "secret".to_string(),
            }),
        )
    }

    fn sample_snapshot() -> Result<ConfigSnapshot> {
        let engine_profile = EngineProfile {
            id: Uuid::new_v4(),
            implementation: "libtorrent".into(),
            listen_port: Some(6881),
            listen_interfaces: Vec::new(),
            ipv6_mode: "disabled".into(),
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            dht: true,
            encryption: "enabled".into(),
            max_active: Some(5),
            max_download_bps: Some(1_000_000),
            max_upload_bps: Some(500_000),
            seed_ratio_limit: None,
            seed_time_limit: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            stats_interval_ms: None,
            alt_speed: AltSpeedConfig::default(),
            sequential_default: false,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            super_seeding: false.into(),
            choking_algorithm: EngineProfile::default_choking_algorithm(),
            seed_choking_algorithm: EngineProfile::default_seed_choking_algorithm(),
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            resume_dir: ".server_root/resume".into(),
            download_root: ".server_root/downloads".into(),
            storage_mode: EngineProfile::default_storage_mode(),
            use_partfile: EngineProfile::default_use_partfile(),
            disk_read_mode: None,
            disk_write_mode: None,
            verify_piece_hashes: EngineProfile::default_verify_piece_hashes(),
            cache_size: None,
            cache_expiry: None,
            coalesce_reads: EngineProfile::default_coalesce_reads(),
            coalesce_writes: EngineProfile::default_coalesce_writes(),
            use_disk_cache_pool: EngineProfile::default_use_disk_cache_pool(),
            tracker: TrackerConfig::default(),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: IpFilterConfig::default(),
            peer_classes: PeerClassesConfig::default(),
            outgoing_port_min: None,
            outgoing_port_max: None,
            peer_dscp: None,
        };
        Ok(ConfigSnapshot {
            revision: 42,
            app_profile: AppProfile {
                id: Uuid::new_v4(),
                instance_name: "demo".into(),
                mode: AppMode::Active,
                auth_mode: revaer_config::AppAuthMode::ApiKey,
                version: 1,
                http_port: 7070,
                bind_addr: "127.0.0.1".parse().map_err(|_| anyhow!("bind addr"))?,
                telemetry: TelemetryConfig {
                    level: Some("info".to_string()),
                    ..TelemetryConfig::default()
                },
                label_policies: Vec::new(),
                immutable_keys: Vec::new(),
            },
            engine_profile: engine_profile.clone(),
            engine_profile_effective: normalize_engine_profile(&engine_profile),
            fs_policy: FsPolicy {
                id: Uuid::new_v4(),
                library_root: ".server_root/library".into(),
                extract: true,
                par2: "disabled".into(),
                flatten: false,
                move_mode: "copy".into(),
                cleanup_keep: Vec::new(),
                cleanup_drop: Vec::new(),
                chmod_file: None,
                chmod_dir: None,
                owner: None,
                group: None,
                umask: None,
                allow_paths: Vec::new(),
            },
        })
    }

    #[tokio::test]
    async fn setup_start_posts_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/admin/setup/start")
                .json_body(json!({"issued_by": "cli", "ttl_seconds": 600}));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "token": "abc123",
                    "expires_at": Utc::now().to_rfc3339()
                }));
        });

        let ctx = context_with(&server, None)?;
        handle_setup_start(
            &ctx,
            SetupStartArgs {
                issued_by: Some("cli".into()),
                ttl_seconds: Some(600),
            },
        )
        .await?;
        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn setup_start_surfaces_problem_details() -> Result<()> {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST).path("/admin/setup/start");
            then.status(400)
                .header("content-type", "application/json")
                .json_body(json!({"title": "bad request", "detail": "missing precondition", "status": 400}));
        });

        let ctx = context_with(&server, None)?;
        let err = handle_setup_start(
            &ctx,
            SetupStartArgs {
                issued_by: None,
                ttl_seconds: None,
            },
        )
        .await
        .err()
        .ok_or_else(|| anyhow!("expected validation error"))?;
        assert!(
            matches!(err, CliError::Validation(message) if message.contains("missing precondition"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn setup_complete_submits_changeset() -> Result<()> {
        let server = MockServer::start_async().await;
        let snapshot = sample_snapshot()?;
        let well_known_snapshot = snapshot.clone();
        server.mock(|when, then| {
            when.method(GET).path("/.well-known/revaer.json");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!(well_known_snapshot));
        });
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/admin/setup/complete")
                .header(HEADER_SETUP_TOKEN, "token-1");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "snapshot": snapshot,
                    "api_key": "admin:secret",
                    "api_key_expires_at": "2025-01-01T00:00:00Z"
                }));
        });

        let ctx = context_with(&server, None)?;
        let args = SetupCompleteArgs {
            token: Some("token-1".to_string()),
            instance: "demo".to_string(),
            bind: "127.0.0.1".to_string(),
            port: 7070,
            resume_dir: PathBuf::from(".server_root/resume"),
            download_root: PathBuf::from(".server_root/downloads"),
            library_root: PathBuf::from(".server_root/library"),
            api_key_label: "label".to_string(),
            api_key_id: Some("admin".to_string()),
            passphrase: Some("secret".to_string()),
        };

        handle_setup_complete(&ctx, args).await?;
        mock.assert();
        Ok(())
    }

    #[test]
    fn build_fs_policy_patch_merges_allow_paths() -> Result<()> {
        let policy = sample_snapshot()?.fs_policy;
        let updated = build_fs_policy_patch(
            policy,
            ".server_root/library",
            ".server_root/downloads",
            ".server_root/downloads",
        );
        assert_eq!(
            updated.allow_paths,
            vec![".server_root/downloads", ".server_root/library"]
        );
        Ok(())
    }

    #[test]
    fn resolve_passphrase_prefers_flag_value() -> CliResult<()> {
        let args = SetupCompleteArgs {
            token: Some("abc".to_string()),
            instance: "demo".to_string(),
            bind: "127.0.0.1".to_string(),
            port: 7070,
            resume_dir: PathBuf::from(".server_root/resume"),
            download_root: PathBuf::from(".server_root/downloads"),
            library_root: PathBuf::from(".server_root/library"),
            api_key_label: "label".to_string(),
            api_key_id: Some("id".to_string()),
            passphrase: Some(" secret ".to_string()),
        };
        let resolved = resolve_passphrase(&args)?;
        assert_eq!(resolved, "secret");
        Ok(())
    }

    #[tokio::test]
    async fn handle_tail_writes_resume_file() -> Result<()> {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let event = revaer_events::EventEnvelope {
            id: 3,
            timestamp: Utc::now(),
            event: revaer_events::Event::TorrentRemoved { torrent_id },
        };
        let payload = serde_json::to_string(&event).map_err(|_| anyhow!("event JSON"))?;
        server.mock(move |when, then| {
            when.method(GET).path("/v1/torrents/events");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(format!("id:3\ndata:{payload}\n\n"));
        });

        let ctx = context_with_key(&server)?;
        let resume_path = std::env::temp_dir().join("revaer-cli-setup-tail.txt");
        let args = crate::cli::TailArgs {
            torrent: Vec::new(),
            event: Vec::new(),
            state: Vec::new(),
            resume_file: Some(resume_path.clone()),
            retry_secs: 0,
        };

        let result = timeout(
            Duration::from_millis(200),
            crate::commands::tail::handle_tail(&ctx, args),
        )
        .await;
        assert!(
            result.is_err(),
            "tail should keep running and be cancelled by timeout"
        );
        let saved = std::fs::read_to_string(resume_path)?;
        assert_eq!(saved.trim(), "3");
        Ok(())
    }
}
