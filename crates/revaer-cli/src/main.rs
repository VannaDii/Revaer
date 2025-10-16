use std::io::{self, IsTerminal};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

use anyhow::{anyhow, Context};
use clap::{Args, Parser, Subcommand};
use rand::distr::Alphanumeric;
use rand::Rng;
use reqwest::{Client, StatusCode, Url};
use revaer_config::{ApiKeyPatch, AppMode, ConfigSnapshot, SecretPatch, SettingsChangeset};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

const HEADER_SETUP_TOKEN: &str = "x-revaer-setup-token";
const HEADER_API_KEY: &str = "x-revaer-api-key";
const DEFAULT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_API_URL: &str = "http://127.0.0.1:7070";

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli).await {
        match err {
            CliError::Validation(message) => {
                eprintln!("error: {message}");
                process::exit(2);
            }
            CliError::Failure(error) => {
                eprintln!("error: {error:#}");
                process::exit(1);
            }
        }
    }
}

async fn run(cli: Cli) -> CliResult<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(cli.timeout))
        .build()
        .map_err(|err| CliError::failure(anyhow!("failed to build HTTP client: {err}")))?;

    let api_key = parse_api_key(cli.api_key)?;

    let ctx = AppContext {
        client,
        base_url: cli.api_url,
        api_key,
    };

    match cli.command {
        Command::Setup(setup) => match setup {
            SetupCommand::Start(args) => handle_setup_start(&ctx, args).await,
            SetupCommand::Complete(args) => handle_setup_complete(&ctx, args).await,
        },
        Command::Settings(settings) => match settings {
            SettingsCommand::Patch(args) => handle_settings_patch(&ctx, args).await,
        },
        Command::Torrents(torrents) => match torrents {
            TorrentCommand::Add(args) => handle_torrent_add(&ctx, args).await,
            TorrentCommand::Remove(args) => handle_torrent_remove(&ctx, args).await,
        },
        Command::Status => handle_status(&ctx).await,
    }
}

#[derive(Parser)]
#[command(name = "revaer", about = "Administrative CLI for the Revaer platform")]
struct Cli {
    #[arg(
        long,
        global = true,
        env = "REVAER_API_URL",
        value_parser = parse_url,
        default_value = DEFAULT_API_URL
    )]
    api_url: Url,
    #[arg(long, global = true, env = "REVAER_API_KEY")]
    api_key: Option<String>,
    #[arg(
        long,
        global = true,
        env = "REVAER_HTTP_TIMEOUT_SECS",
        default_value_t = DEFAULT_TIMEOUT_SECS
    )]
    timeout: u64,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(subcommand)]
    Setup(SetupCommand),
    #[command(subcommand)]
    Settings(SettingsCommand),
    #[command(subcommand)]
    Torrents(TorrentCommand),
    Status,
}

#[derive(Subcommand)]
enum SetupCommand {
    Start(SetupStartArgs),
    Complete(SetupCompleteArgs),
}

#[derive(Args)]
struct SetupStartArgs {
    #[arg(long)]
    issued_by: Option<String>,
    #[arg(long)]
    ttl_seconds: Option<u64>,
}

#[derive(Args)]
struct SetupCompleteArgs {
    #[arg(long, env = "REVAER_SETUP_TOKEN")]
    token: Option<String>,
    #[arg(long)]
    instance: String,
    #[arg(long)]
    bind: String,
    #[arg(long, default_value_t = 7070)]
    port: u16,
    #[arg(long)]
    resume_dir: PathBuf,
    #[arg(long)]
    download_root: PathBuf,
    #[arg(long)]
    library_root: PathBuf,
    #[arg(long)]
    api_key_label: String,
    #[arg(long)]
    api_key_id: Option<String>,
    #[arg(long)]
    passphrase: Option<String>,
}

#[derive(Subcommand)]
enum SettingsCommand {
    Patch(SettingsPatchArgs),
}

#[derive(Subcommand)]
enum TorrentCommand {
    Add(TorrentAddArgs),
    Remove(TorrentRemoveArgs),
}

#[derive(Args)]
struct TorrentAddArgs {
    #[arg(long, help = "Human-readable torrent name")]
    name: String,
    #[arg(long, help = "Optional torrent identifier (defaults to random UUID)")]
    id: Option<Uuid>,
}

#[derive(Args)]
struct TorrentRemoveArgs {
    #[arg(help = "Torrent identifier")]
    id: Uuid,
}

#[derive(Args)]
struct SettingsPatchArgs {
    #[arg(short = 'f', long = "file")]
    file: PathBuf,
}

struct AppContext {
    client: Client,
    base_url: Url,
    api_key: Option<ApiKeyCredential>,
}

#[derive(Clone)]
struct ApiKeyCredential {
    key_id: String,
    secret: String,
}

impl ApiKeyCredential {
    fn header_value(&self) -> String {
        format!("{}:{}", self.key_id, self.secret)
    }
}

#[derive(Debug)]
enum CliError {
    Validation(String),
    Failure(anyhow::Error),
}

type CliResult<T> = Result<T, CliError>;

impl CliError {
    fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    fn failure(error: impl Into<anyhow::Error>) -> Self {
        Self::Failure(error.into())
    }
}

fn parse_url(input: &str) -> Result<Url, String> {
    input
        .parse::<Url>()
        .map_err(|err| format!("invalid URL '{input}': {err}"))
}

fn parse_api_key(input: Option<String>) -> CliResult<Option<ApiKeyCredential>> {
    let Some(raw) = input else {
        return Ok(None);
    };

    let trimmed = raw.trim();
    let (key_id, secret) = trimmed
        .split_once(':')
        .ok_or_else(|| CliError::validation("API key must be provided as key_id:secret"))?;

    if key_id.trim().is_empty() || secret.trim().is_empty() {
        return Err(CliError::validation(
            "API key components cannot be empty strings",
        ));
    }

    Ok(Some(ApiKeyCredential {
        key_id: key_id.trim().to_string(),
        secret: secret.trim().to_string(),
    }))
}

async fn handle_setup_start(ctx: &AppContext, args: SetupStartArgs) -> CliResult<()> {
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

async fn handle_setup_complete(ctx: &AppContext, args: SetupCompleteArgs) -> CliResult<()> {
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

async fn handle_torrent_add(ctx: &AppContext, args: TorrentAddArgs) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let id = args.id.unwrap_or_else(Uuid::new_v4);
    let payload = json!({
        "id": id,
        "name": args.name,
    });

    let url = ctx
        .base_url
        .join("/admin/torrents")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&payload)
        .send()
        .await
        .map_err(|err| CliError::failure(anyhow!("request to /admin/torrents failed: {err}")))?;

    if response.status().is_success() {
        println!("Torrent submission requested (id: {id})");
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

async fn handle_torrent_remove(ctx: &AppContext, args: TorrentRemoveArgs) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;
    let id = args.id;

    let url = ctx
        .base_url
        .join(&format!("/admin/torrents/{id}"))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .delete(url)
        .header(HEADER_API_KEY, creds.header_value())
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!("request to /admin/torrents/{id} failed: {err}"))
        })?;

    if response.status().is_success() {
        println!("Torrent removal requested (id: {id})");
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

async fn handle_settings_patch(ctx: &AppContext, args: SettingsPatchArgs) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let payload = std::fs::read_to_string(&args.file)
        .with_context(|| format!("failed to read {}", args.file.display()))
        .map_err(CliError::failure)?;

    let changeset: SettingsChangeset = serde_json::from_str(&payload)
        .map_err(|err| CliError::failure(anyhow!("settings file is not valid JSON: {err}")))?;

    let url = ctx
        .base_url
        .join("/admin/settings")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .patch(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&changeset)
        .send()
        .await
        .map_err(|err| CliError::failure(anyhow!("request to /admin/settings failed: {err}")))?;

    if response.status().is_success() {
        println!("Settings patch applied.");
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

async fn fetch_torrent_catalog(
    ctx: &AppContext,
    creds: &ApiKeyCredential,
) -> CliResult<Vec<TorrentStatusSummary>> {
    let url = ctx
        .base_url
        .join("/admin/torrents")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .get(url)
        .header(HEADER_API_KEY, creds.header_value())
        .send()
        .await
        .map_err(|err| CliError::failure(anyhow!("request to /admin/torrents failed: {err}")))?;

    if response.status().is_success() {
        response
            .json::<Vec<TorrentStatusSummary>>()
            .await
            .map_err(|err| {
                CliError::failure(anyhow!("failed to parse torrent status response: {err}"))
            })
    } else {
        Err(classify_problem(response).await)
    }
}

async fn handle_status(ctx: &AppContext) -> CliResult<()> {
    let health = {
        let url = ctx
            .base_url
            .join("/health")
            .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;
        ctx.client
            .get(url)
            .send()
            .await
            .map_err(|err| CliError::failure(anyhow!("request to /health failed: {err}")))?
            .json::<HealthResponse>()
            .await
            .map_err(|err| CliError::failure(anyhow!("failed to parse /health response: {err}")))?
    };

    let snapshot = {
        let url = ctx
            .base_url
            .join("/.well-known/revaer.json")
            .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;
        ctx.client
            .get(url)
            .send()
            .await
            .map_err(|err| {
                CliError::failure(anyhow!("request to /.well-known/revaer.json failed: {err}"))
            })?
            .json::<ConfigSnapshot>()
            .await
            .map_err(|err| {
                CliError::failure(anyhow!("failed to parse configuration snapshot: {err}"))
            })?
    };

    println!("mode: {}", health.mode.as_str());
    println!("instance: {}", snapshot.app_profile.instance_name);
    println!("revision: {}", snapshot.revision);
    println!(
        "health: {} (db: {}, revision: {:?})",
        health.status, health.database.status, health.database.revision
    );

    if let Some(creds) = &ctx.api_key {
        let torrents = fetch_torrent_catalog(ctx, creds).await?;
        if torrents.is_empty() {
            println!("torrents: none tracked");
        } else {
            println!("torrents ({}):", torrents.len());
            for torrent in torrents.iter().take(10) {
                let name = torrent.name.as_deref().unwrap_or("<unnamed>");
                let failure = torrent
                    .failure_message
                    .as_deref()
                    .map_or(String::new(), |msg| format!(" (failure: {msg})"));
                println!(
                    "- {} ({}) [{}] {:.1}%{}",
                    torrent.id, name, torrent.state, torrent.progress.percent_complete, failure
                );
            }
            if torrents.len() > 10 {
                println!("... and {} more torrents", torrents.len() - 10);
            }
        }
    } else {
        println!("torrents: (authentication required; pass --api-key)");
    }

    Ok(())
}

fn build_fs_policy_patch(library_root: &str, download_root: &str, resume_dir: &str) -> Value {
    let mut allow_paths = vec![download_root.to_string(), library_root.to_string()];
    if !allow_paths.iter().any(|p| p == resume_dir) {
        allow_paths.push(resume_dir.to_string());
    }

    json!({
        "library_root": library_root,
        "allow_paths": allow_paths,
    })
}

fn resolve_passphrase(args: &SetupCompleteArgs) -> CliResult<String> {
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

fn path_to_string(path: &Path) -> CliResult<String> {
    path.to_str().map(str::to_string).ok_or_else(|| {
        CliError::validation(format!("path '{}' is not valid UTF-8", path.display()))
    })
}

fn random_string(len: usize) -> String {
    let mut rng = rand::rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric) as char)
        .take(len)
        .collect()
}

async fn classify_problem(response: reqwest::Response) -> CliError {
    let status = response.status();
    let bytes = response.bytes().await.unwrap_or_default();

    let body_text = String::from_utf8_lossy(&bytes).to_string();
    let problem = serde_json::from_slice::<ProblemDetails>(&bytes).ok();

    let message = problem
        .as_ref()
        .and_then(|p| p.detail.clone())
        .unwrap_or_else(|| {
            problem
                .as_ref()
                .map_or_else(|| body_text.trim().to_string(), |p| p.title.clone())
        });

    if matches!(
        status,
        StatusCode::BAD_REQUEST | StatusCode::CONFLICT | StatusCode::UNPROCESSABLE_ENTITY
    ) {
        CliError::validation(message)
    } else {
        let detail = if let Some(problem) = problem {
            format!("{} (status {})", message, problem.status)
        } else if !body_text.is_empty() {
            format!("{message} (status {status})")
        } else {
            format!("request failed with status {status}")
        };
        CliError::failure(anyhow!(detail))
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

#[derive(Debug, Deserialize)]
struct ProblemDetails {
    title: String,
    status: u16,
    detail: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HealthComponent {
    status: String,
    revision: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
    mode: AppMode,
    database: HealthComponent,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TorrentProgressStatus {
    bytes_downloaded: u64,
    bytes_total: u64,
    percent_complete: f64,
}

#[derive(Debug, Deserialize)]
struct TorrentStatusSummary {
    id: Uuid,
    name: Option<String>,
    state: String,
    #[serde(default)]
    failure_message: Option<String>,
    progress: TorrentProgressStatus,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use httpmock::prelude::*;
    use reqwest::Client;
    use serde_json::json;

    fn context_for(server: &MockServer) -> AppContext {
        AppContext {
            client: Client::new(),
            base_url: server.base_url().parse().expect("valid URL"),
            api_key: Some(ApiKeyCredential {
                key_id: "key".to_string(),
                secret: "secret".to_string(),
            }),
        }
    }

    #[tokio::test]
    async fn torrent_add_issues_post_request() {
        let server = MockServer::start_async().await;
        let id = Uuid::new_v4();
        let name = "demo.torrent";

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/admin/torrents")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "id": id,
                    "name": name,
                }));
            then.status(202);
        });

        let ctx = context_for(&server);
        let args = TorrentAddArgs {
            name: name.to_string(),
            id: Some(id),
        };

        handle_torrent_add(&ctx, args)
            .await
            .expect("torrent add should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn torrent_remove_issues_delete_request() {
        let server = MockServer::start_async().await;
        let id = Uuid::new_v4();

        let path = format!("/admin/torrents/{id}");
        let mock = server.mock(move |when, then| {
            when.method(DELETE)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret");
            then.status(204);
        });

        let ctx = context_for(&server);
        let args = TorrentRemoveArgs { id };

        handle_torrent_remove(&ctx, args)
            .await
            .expect("torrent remove should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn status_fetches_torrent_catalog_when_authenticated() {
        let server = MockServer::start_async().await;

        let health_mock = server.mock(|when, then| {
            when.method(GET).path("/health");
            then.status(200).json_body(json!({
                "status": "ok",
                "mode": "active",
                "database": { "status": "ok", "revision": 42 }
            }));
        });

        let snapshot_mock = server.mock(|when, then| {
            when.method(GET).path("/.well-known/revaer.json");
            then.status(200).json_body(json!({
                "revision": 42,
                "app_profile": {
                    "id": Uuid::new_v4(),
                    "instance_name": "demo",
                    "mode": "active",
                    "version": 1,
                    "http_port": 7070,
                    "bind_addr": "127.0.0.1",
                    "telemetry": json!({}),
                    "features": json!({}),
                    "immutable_keys": json!({}),
                },
                "engine_profile": {
                    "id": Uuid::new_v4(),
                    "implementation": "libtorrent",
                    "listen_port": 6881,
                    "dht": true,
                    "encryption": "prefer",
                    "max_active": null,
                    "max_download_bps": null,
                    "max_upload_bps": null,
                    "sequential_default": false,
                    "resume_dir": "/tmp/resume",
                    "download_root": "/downloads",
                    "tracker": json!([]),
                },
                "fs_policy": {
                    "id": Uuid::new_v4(),
                    "library_root": "/library",
                    "extract": false,
                    "par2": "disabled",
                    "flatten": false,
                    "move_mode": "copy",
                    "cleanup_keep": json!([]),
                    "cleanup_drop": json!([]),
                    "chmod_file": null,
                    "chmod_dir": null,
                    "owner": null,
                    "group": null,
                    "umask": null,
                    "allow_paths": json!([]),
                }
            }));
        });

        let torrent_id = Uuid::new_v4();
        let torrents_mock = server.mock(move |when, then| {
            when.method(GET)
                .path("/admin/torrents")
                .header(HEADER_API_KEY, "key:secret");
            then.status(200).json_body(json!([
                {
                    "id": torrent_id,
                    "name": "demo",
                    "state": "downloading",
                    "failure_message": null,
                    "progress": {
                        "bytes_downloaded": 512,
                        "bytes_total": 1024,
                        "percent_complete": 50.0
                    },
                    "files": null,
                    "library_path": null,
                    "last_updated": Utc::now().to_rfc3339()
                }
            ]));
        });

        let ctx = context_for(&server);
        handle_status(&ctx)
            .await
            .expect("status command should succeed");

        health_mock.assert();
        snapshot_mock.assert();
        torrents_mock.assert();
    }
}
