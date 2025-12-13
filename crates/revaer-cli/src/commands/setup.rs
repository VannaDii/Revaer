use anyhow::anyhow;
use revaer_config::{ApiKeyPatch, ConfigSnapshot, SecretPatch, SettingsChangeset};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
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
        let payload = SetupStartPayload {
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

    let api_key_id = args.api_key_id.clone().unwrap_or_else(|| random_string(24));
    let api_key_secret = random_string(48);

    let changeset = SettingsChangeset {
        app_profile: Some(json!({
            "instance_name": args.instance,
            "bind_addr": bind_addr.to_string(),
            "http_port": i64::from(args.port)
        })),
        engine_profile: Some(json!({
            "implementation": "libtorrent",
            "resume_dir": resume_dir,
            "download_root": download_root
        })),
        fs_policy: Some(build_fs_policy_patch(
            &library_root,
            &download_root,
            &resume_dir,
        )),
        api_keys: vec![ApiKeyPatch::Upsert {
            key_id: api_key_id.clone(),
            label: Some(args.api_key_label.clone()),
            enabled: Some(true),
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
        let snapshot = response.json::<ConfigSnapshot>().await.map_err(|err| {
            CliError::failure(anyhow!("failed to parse setup completion response: {err}"))
        })?;
        let instance_name = &snapshot.app_profile.instance_name;
        println!("Setup complete for instance '{instance_name}'.");
        println!("API key created (store securely): {api_key_id}:{api_key_secret}");
        Ok(())
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
    library_root: &str,
    download_root: &str,
    resume_dir: &str,
) -> Value {
    let mut allow_paths = vec![download_root.to_string(), library_root.to_string()];
    if !allow_paths.iter().any(|p| p == resume_dir) {
        allow_paths.push(resume_dir.to_string());
    }

    json!({
        "library_root": library_root,
        "allow_paths": allow_paths,
    })
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

#[derive(Debug, Serialize, Deserialize)]
struct SetupStartPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    issued_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ttl_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct SetupStartResponse {
    token: String,
    expires_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use httpmock::prelude::*;
    use reqwest::Client;
    use revaer_config::{AppMode, AppProfile, EngineProfile, FsPolicy, normalize_engine_profile};
    use std::path::PathBuf;
    use tokio::time::{Duration, timeout};
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
        let engine_profile = EngineProfile {
            id: Uuid::new_v4(),
            implementation: "libtorrent".into(),
            listen_port: Some(6881),
            dht: true,
            encryption: "enabled".into(),
            max_active: Some(5),
            max_download_bps: Some(1_000_000),
            max_upload_bps: Some(500_000),
            sequential_default: false,
            resume_dir: "/var/resume".into(),
            download_root: "/var/downloads".into(),
            tracker: json!({}),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
        };
        ConfigSnapshot {
            revision: 42,
            app_profile: AppProfile {
                id: Uuid::new_v4(),
                instance_name: "demo".into(),
                mode: AppMode::Active,
                version: 1,
                http_port: 7070,
                bind_addr: "127.0.0.1".parse().unwrap(),
                telemetry: json!({"level": "info"}),
                features: json!({}),
                immutable_keys: json!([]),
            },
            engine_profile: engine_profile.clone(),
            engine_profile_effective: normalize_engine_profile(&engine_profile),
            fs_policy: FsPolicy {
                id: Uuid::new_v4(),
                library_root: "/library".into(),
                extract: true,
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
    async fn setup_start_posts_payload() {
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

        let ctx = context_with(&server, None);
        handle_setup_start(
            &ctx,
            SetupStartArgs {
                issued_by: Some("cli".into()),
                ttl_seconds: Some(600),
            },
        )
        .await
        .expect("setup start should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn setup_start_surfaces_problem_details() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(POST).path("/admin/setup/start");
            then.status(400)
                .header("content-type", "application/json")
                .json_body(json!({"title": "bad request", "detail": "missing precondition", "status": 400}));
        });

        let ctx = context_with(&server, None);
        let err = handle_setup_start(
            &ctx,
            SetupStartArgs {
                issued_by: None,
                ttl_seconds: None,
            },
        )
        .await
        .expect_err("validation error expected");
        assert!(
            matches!(err, CliError::Validation(message) if message.contains("missing precondition"))
        );
    }

    #[tokio::test]
    async fn setup_complete_submits_changeset() {
        let server = MockServer::start_async().await;
        let snapshot = sample_snapshot();
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/admin/setup/complete")
                .header(HEADER_SETUP_TOKEN, "token-1");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!(snapshot));
        });

        let ctx = context_with(&server, None);
        let args = SetupCompleteArgs {
            token: Some("token-1".to_string()),
            instance: "demo".to_string(),
            bind: "127.0.0.1".to_string(),
            port: 7070,
            resume_dir: PathBuf::from("/tmp/resume"),
            download_root: PathBuf::from("/tmp/download"),
            library_root: PathBuf::from("/tmp/library"),
            api_key_label: "label".to_string(),
            api_key_id: Some("admin".to_string()),
            passphrase: Some("secret".to_string()),
        };

        handle_setup_complete(&ctx, args)
            .await
            .expect("setup complete should succeed");
        mock.assert();
    }

    #[test]
    fn build_fs_policy_patch_merges_allow_paths() {
        let patch = build_fs_policy_patch("/library", "/downloads", "/downloads");
        let allow_paths = patch
            .get("allow_paths")
            .and_then(Value::as_array)
            .expect("allow_paths array");
        let values: Vec<&str> = allow_paths
            .iter()
            .map(|value| value.as_str().expect("string"))
            .collect();
        assert_eq!(values, vec!["/downloads", "/library"]);
    }

    #[test]
    fn resolve_passphrase_prefers_flag_value() -> CliResult<()> {
        let args = SetupCompleteArgs {
            token: Some("abc".to_string()),
            instance: "demo".to_string(),
            bind: "127.0.0.1".to_string(),
            port: 7070,
            resume_dir: PathBuf::from("/tmp/resume"),
            download_root: PathBuf::from("/tmp/download"),
            library_root: PathBuf::from("/tmp/library"),
            api_key_label: "label".to_string(),
            api_key_id: Some("id".to_string()),
            passphrase: Some(" secret ".to_string()),
        };
        let resolved = resolve_passphrase(&args)?;
        assert_eq!(resolved, "secret");
        Ok(())
    }

    #[tokio::test]
    async fn handle_tail_writes_resume_file() {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let event = revaer_events::EventEnvelope {
            id: 3,
            timestamp: Utc::now(),
            event: revaer_events::Event::TorrentRemoved { torrent_id },
        };
        let payload = serde_json::to_string(&event).expect("event JSON");
        server.mock(move |when, then| {
            when.method(GET).path("/v1/torrents/events");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(format!("id:3\ndata:{payload}\n\n"));
        });

        let ctx = context_with_key(&server);
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
        let saved = std::fs::read_to_string(resume_path).expect("resume file");
        assert_eq!(saved.trim(), "3");
    }
}
