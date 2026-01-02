use anyhow::{Context, anyhow};
use revaer_config::ConfigSnapshot;

use crate::cli::{ConfigSetArgs, OutputFormat};
use crate::client::{AppContext, CliError, CliResult, HEADER_API_KEY, classify_problem};
use crate::output::render_config_snapshot;

pub(crate) async fn handle_config_get(ctx: &AppContext, format: OutputFormat) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let url = ctx
        .base_url
        .join("/v1/config")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .get(url)
        .header(HEADER_API_KEY, creds.header_value())
        .send()
        .await
        .map_err(|err| CliError::failure(anyhow!("request to /v1/config failed: {err}")))?;

    if response.status().is_success() {
        let snapshot = response
            .json::<ConfigSnapshot>()
            .await
            .map_err(|err| CliError::failure(anyhow!("failed to parse config snapshot: {err}")))?;
        render_config_snapshot(&snapshot, format)?;
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_config_set(ctx: &AppContext, args: ConfigSetArgs) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let payload = std::fs::read_to_string(&args.file)
        .with_context(|| format!("failed to read {}", args.file.display()))
        .map_err(CliError::failure)?;

    let changeset: revaer_config::SettingsChangeset = serde_json::from_str(&payload)
        .map_err(|err| CliError::failure(anyhow!("settings file is not valid JSON: {err}")))?;

    let url = ctx
        .base_url
        .join("/v1/config")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .patch(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&changeset)
        .send()
        .await
        .map_err(|err| CliError::failure(anyhow!("request to /v1/config failed: {err}")))?;

    if response.status().is_success() {
        println!("Settings patch applied.");
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Result, anyhow};
    use httpmock::prelude::*;
    use reqwest::Client;
    use serde_json::json;
    use std::{fs, path::PathBuf};
    use uuid::Uuid;

    use crate::client::ApiKeyCredential;

    fn repo_root() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for ancestor in manifest_dir.ancestors() {
            if ancestor.join("AGENT.md").is_file() {
                return ancestor.to_path_buf();
            }
        }
        manifest_dir
    }

    fn server_root() -> Result<PathBuf> {
        let root = repo_root().join(".server_root");
        fs::create_dir_all(&root)?;
        Ok(root)
    }

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
        let engine_profile = revaer_config::EngineProfile {
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
            encryption: "prefer".into(),
            max_active: Some(4),
            max_download_bps: None,
            max_upload_bps: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            stats_interval_ms: None,
            alt_speed: revaer_config::engine_profile::AltSpeedConfig::default(),
            sequential_default: false,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            super_seeding: false.into(),
            choking_algorithm: revaer_config::EngineProfile::default_choking_algorithm(),
            seed_choking_algorithm: revaer_config::EngineProfile::default_seed_choking_algorithm(),
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            resume_dir: ".server_root/resume".into(),
            download_root: ".server_root/downloads".into(),
            storage_mode: revaer_config::EngineProfile::default_storage_mode(),
            use_partfile: revaer_config::EngineProfile::default_use_partfile(),
            disk_read_mode: None,
            disk_write_mode: None,
            verify_piece_hashes: revaer_config::EngineProfile::default_verify_piece_hashes(),
            cache_size: None,
            cache_expiry: None,
            coalesce_reads: revaer_config::EngineProfile::default_coalesce_reads(),
            coalesce_writes: revaer_config::EngineProfile::default_coalesce_writes(),
            use_disk_cache_pool: revaer_config::EngineProfile::default_use_disk_cache_pool(),
            tracker: revaer_config::engine_profile::TrackerConfig::default(),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: revaer_config::engine_profile::IpFilterConfig::default(),
            peer_classes: revaer_config::engine_profile::PeerClassesConfig::default(),
            outgoing_port_min: None,
            outgoing_port_max: None,
            peer_dscp: None,
        };
        Ok(ConfigSnapshot {
            revision: 1,
            app_profile: revaer_config::AppProfile {
                id: Uuid::new_v4(),
                instance_name: "demo".into(),
                mode: revaer_config::AppMode::Active,
                auth_mode: revaer_config::AppAuthMode::ApiKey,
                version: 1,
                http_port: 7070,
                bind_addr: "127.0.0.1".parse().map_err(|_| anyhow!("bind addr"))?,
                telemetry: revaer_config::TelemetryConfig::default(),
                label_policies: Vec::new(),
                immutable_keys: Vec::new(),
            },
            engine_profile: engine_profile.clone(),
            engine_profile_effective: revaer_config::normalize_engine_profile(&engine_profile),
            fs_policy: revaer_config::FsPolicy {
                id: Uuid::new_v4(),
                library_root: ".server_root/library".into(),
                extract: false,
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
    async fn config_set_sends_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let mock = server.mock(|when, then| {
            when.method(PATCH)
                .path("/v1/config")
                .header(HEADER_API_KEY, "key:secret");
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        let file_path = server_root()?.join(format!("revaer-cli-config-{}.json", Uuid::new_v4()));
        let mut app_profile = sample_snapshot()?.app_profile;
        app_profile.id = Uuid::parse_str("00000000-0000-0000-0000-000000000001")
            .map_err(|_| anyhow!("valid app profile id"))?;
        let changeset = revaer_config::SettingsChangeset {
            app_profile: Some(app_profile),
            engine_profile: None,
            fs_policy: None,
            api_keys: Vec::new(),
            secrets: Vec::new(),
        };
        let payload = serde_json::to_string(&changeset)
            .map_err(|_| anyhow!("serialize settings changeset"))?;
        std::fs::write(&file_path, payload)?;

        handle_config_set(
            &ctx,
            ConfigSetArgs {
                file: file_path.clone(),
            },
        )
        .await?;
        mock.assert();
        std::fs::remove_file(&file_path)?;
        Ok(())
    }

    #[tokio::test]
    async fn config_get_fetches_snapshot() -> Result<()> {
        let server = MockServer::start_async().await;
        let snapshot = sample_snapshot()?;
        let mock = server.mock(move |when, then| {
            when.method(GET)
                .path("/v1/config")
                .header(HEADER_API_KEY, "key:secret");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!(snapshot));
        });

        let ctx = context_with_key(&server)?;
        handle_config_get(&ctx, OutputFormat::Table).await?;
        mock.assert();
        Ok(())
    }
}
