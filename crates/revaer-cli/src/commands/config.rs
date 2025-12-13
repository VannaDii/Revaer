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
    use httpmock::prelude::*;
    use reqwest::Client;
    use serde_json::json;
    use uuid::Uuid;

    use crate::client::ApiKeyCredential;

    fn context_with(server: &MockServer, api_key: Option<ApiKeyCredential>) -> AppContext {
        AppContext {
            client: Client::new(),
            base_url: server.base_url().parse().expect("valid URL"),
            api_key,
        }
    }

    fn context_with_key(server: &MockServer) -> AppContext {
        context_with(
            server,
            Some(ApiKeyCredential {
                key_id: "key".to_string(),
                secret: "secret".to_string(),
            }),
        )
    }

    fn sample_snapshot() -> ConfigSnapshot {
        let engine_profile = revaer_config::EngineProfile {
            id: Uuid::new_v4(),
            implementation: "libtorrent".into(),
            listen_port: Some(6881),
            dht: true,
            encryption: "prefer".into(),
            max_active: Some(4),
            max_download_bps: None,
            max_upload_bps: None,
            sequential_default: false,
            resume_dir: "/tmp/resume".into(),
            download_root: "/tmp/downloads".into(),
            tracker: json!({}),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: json!({}),
        };
        ConfigSnapshot {
            revision: 1,
            app_profile: revaer_config::AppProfile {
                id: Uuid::new_v4(),
                instance_name: "demo".into(),
                mode: revaer_config::AppMode::Active,
                version: 1,
                http_port: 7070,
                bind_addr: "127.0.0.1".parse().unwrap(),
                telemetry: json!({}),
                features: json!({}),
                immutable_keys: json!([]),
            },
            engine_profile: engine_profile.clone(),
            engine_profile_effective: revaer_config::normalize_engine_profile(&engine_profile),
            fs_policy: revaer_config::FsPolicy {
                id: Uuid::new_v4(),
                library_root: "/library".into(),
                extract: false,
                par2: "disabled".into(),
                flatten: false,
                move_mode: "copy".into(),
                cleanup_keep: json!([]),
                cleanup_drop: json!([]),
                chmod_file: None,
                chmod_dir: None,
                owner: None,
                group: None,
                umask: None,
                allow_paths: json!([]),
            },
        }
    }

    #[tokio::test]
    async fn config_set_sends_payload() {
        let server = MockServer::start_async().await;
        let mock = server.mock(|when, then| {
            when.method(PATCH)
                .path("/v1/config")
                .header(HEADER_API_KEY, "key:secret");
            then.status(200);
        });

        let ctx = context_with_key(&server);
        let file_path =
            std::env::temp_dir().join(format!("revaer-cli-config-{}.json", Uuid::new_v4()));
        std::fs::write(
            &file_path,
            r#"{
                "app_profile": { "instance_name": "custom" },
                "api_keys": [],
                "secrets": []
            }"#,
        )
        .expect("write settings");

        handle_config_set(
            &ctx,
            ConfigSetArgs {
                file: file_path.clone(),
            },
        )
        .await
        .expect("settings patch should succeed");
        mock.assert();
        let _ = std::fs::remove_file(file_path);
    }

    #[tokio::test]
    async fn config_get_fetches_snapshot() {
        let server = MockServer::start_async().await;
        let snapshot = sample_snapshot();
        let mock = server.mock(move |when, then| {
            when.method(GET)
                .path("/v1/config")
                .header(HEADER_API_KEY, "key:secret");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!(snapshot));
        });

        let ctx = context_with_key(&server);
        handle_config_get(&ctx, OutputFormat::Table)
            .await
            .expect("config get should succeed");
        mock.assert();
    }
}
