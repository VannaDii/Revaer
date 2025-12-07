//! Command-line client for interacting with a Revaer server instance.

use std::convert::TryFrom;
use std::env;
use std::io::{self, IsTerminal};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, anyhow};
use base64::{Engine as _, engine::general_purpose};
use clap::{Args, Parser, Subcommand, ValueEnum};
use rand::Rng;
use rand::distr::Alphanumeric;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, StatusCode, Url};
use revaer_api::models::{
    ProblemDetails, TorrentAction as ApiTorrentAction, TorrentCreateRequest, TorrentDetail,
    TorrentListResponse, TorrentSelectionRequest, TorrentStateKind,
};
use revaer_config::{ApiKeyPatch, ConfigSnapshot, SecretPatch, SettingsChangeset};
use revaer_events::EventEnvelope;
use revaer_torrent_core::{FilePriority, FilePriorityOverride};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use tokio::time::sleep;
use uuid::Uuid;

const HEADER_SETUP_TOKEN: &str = "x-revaer-setup-token";
const HEADER_API_KEY: &str = "x-revaer-api-key";
const HEADER_REQUEST_ID: &str = "x-request-id";
const HEADER_LAST_EVENT_ID: &str = "Last-Event-ID";
const DEFAULT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_API_URL: &str = "http://127.0.0.1:7070";

#[derive(Clone)]
struct CliDependencies {
    client: Client,
    telemetry: Option<TelemetryEmitter>,
}

impl CliDependencies {
    fn from_env(cli: &Cli, trace_id: &str) -> CliResult<Self> {
        let mut default_headers = HeaderMap::new();
        let request_id = HeaderValue::from_str(trace_id).map_err(|_| {
            CliError::failure(anyhow!("trace identifier contains invalid characters"))
        })?;
        default_headers.insert(HEADER_REQUEST_ID, request_id);

        let client = Client::builder()
            .timeout(Duration::from_secs(cli.timeout))
            .default_headers(default_headers)
            .build()
            .map_err(|err| CliError::failure(anyhow!("failed to build HTTP client: {err}")))?;

        Ok(Self {
            client,
            telemetry: TelemetryEmitter::from_env(),
        })
    }
}

/// Parses CLI arguments, executes the requested command, and handles
/// user-facing telemetry emission. Returns the process exit code.
pub async fn run() -> i32 {
    let cli = Cli::parse();
    let command_name = command_label(&cli.command);
    let trace_id = Uuid::new_v4().to_string();
    let deps = match CliDependencies::from_env(&cli, &trace_id) {
        Ok(deps) => deps,
        Err(err) => {
            eprintln!("error: {}", err.display_message());
            return err.exit_code();
        }
    };
    let telemetry = deps.telemetry.clone();

    let result = dispatch(cli, &deps).await;

    let (exit_code, message, outcome) = match result {
        Ok(()) => (0, None, "success"),
        Err(err) => {
            let exit_code = err.exit_code();
            let message = err.display_message();
            eprintln!("error: {message}");
            (exit_code, Some(message), "error")
        }
    };

    if let Some(emitter) = &telemetry {
        emitter
            .emit(
                &trace_id,
                command_name,
                outcome,
                exit_code,
                message.as_deref(),
            )
            .await;
    }

    exit_code
}

async fn dispatch(cli: Cli, deps: &CliDependencies) -> CliResult<()> {
    let api_key = parse_api_key(cli.api_key)?;

    let ctx = AppContext {
        client: deps.client.clone(),
        base_url: cli.api_url,
        api_key,
    };

    match cli.command {
        Command::Setup(setup) => match setup {
            SetupCommand::Start(args) => handle_setup_start(&ctx, args).await,
            SetupCommand::Complete(args) => handle_setup_complete(&ctx, args).await,
        },
        Command::Config(config) => match config {
            ConfigCommand::Get(_) => handle_config_get(&ctx, cli.output).await,
            ConfigCommand::Set(args) => handle_config_set(&ctx, args).await,
        },
        Command::Settings(settings) => match settings {
            SettingsCommand::Patch(args) => handle_config_set(&ctx, args).await,
        },
        Command::Torrent(torrents) => match torrents {
            TorrentCommand::Add(args) => handle_torrent_add(&ctx, args).await,
            TorrentCommand::Remove(args) => handle_torrent_remove(&ctx, args).await,
        },
        Command::Ls(args) => handle_torrent_list(&ctx, args, cli.output).await,
        Command::Status(args) => handle_torrent_status(&ctx, args, cli.output).await,
        Command::Select(args) => handle_torrent_select(&ctx, args).await,
        Command::Action(args) => handle_torrent_action(&ctx, args).await,
        Command::Tail(args) => handle_tail(&ctx, args).await,
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
    #[arg(
        long = "output",
        alias = "format",
        global = true,
        value_enum,
        default_value_t = OutputFormat::Table,
        help = "Select output format for commands that render structured data"
    )]
    output: OutputFormat,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(subcommand)]
    Setup(SetupCommand),
    #[command(subcommand)]
    Config(ConfigCommand),
    #[command(subcommand)]
    Settings(SettingsCommand),
    #[command(subcommand)]
    Torrent(TorrentCommand),
    Ls(TorrentListArgs),
    Status(TorrentStatusArgs),
    Select(TorrentSelectArgs),
    Action(TorrentActionArgs),
    Tail(TailArgs),
}

#[derive(Subcommand)]
enum SetupCommand {
    Start(SetupStartArgs),
    Complete(SetupCompleteArgs),
}

#[derive(Subcommand)]
enum ConfigCommand {
    Get(ConfigGetArgs),
    Set(ConfigSetArgs),
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

#[derive(Default, Args)]
struct ConfigGetArgs {}

#[derive(Subcommand)]
enum SettingsCommand {
    Patch(ConfigSetArgs),
}

#[derive(Subcommand)]
enum TorrentCommand {
    Add(TorrentAddArgs),
    Remove(TorrentRemoveArgs),
}

#[derive(Args)]
struct TorrentAddArgs {
    #[arg(help = "Magnet URI or path to a .torrent file")]
    source: String,
    #[arg(long, help = "Optional human-readable torrent name")]
    name: Option<String>,
    #[arg(long, help = "Optional torrent identifier (defaults to random UUID)")]
    id: Option<Uuid>,
}

#[derive(Args)]
struct TorrentRemoveArgs {
    #[arg(help = "Torrent identifier")]
    id: Uuid,
}

#[derive(Args)]
struct ConfigSetArgs {
    #[arg(short = 'f', long = "file")]
    file: PathBuf,
}

#[derive(Args, Default)]
struct TorrentListArgs {
    #[arg(long)]
    limit: Option<u32>,
    #[arg(long)]
    cursor: Option<String>,
    #[arg(long)]
    state: Option<String>,
    #[arg(long)]
    tracker: Option<String>,
    #[arg(long)]
    extension: Option<String>,
    #[arg(long)]
    tags: Option<String>,
    #[arg(long)]
    name: Option<String>,
}

#[derive(Args)]
struct TorrentStatusArgs {
    #[arg(help = "Torrent identifier")]
    id: Uuid,
}

#[derive(Clone, Debug)]
struct FilePriorityOverrideArg {
    index: u32,
    priority: FilePriority,
}

#[derive(Args, Default)]
struct TorrentSelectArgs {
    #[arg(help = "Torrent identifier")]
    id: Uuid,
    #[arg(long, value_delimiter = ',')]
    include: Vec<String>,
    #[arg(long, value_delimiter = ',')]
    exclude: Vec<String>,
    #[arg(long)]
    skip_fluff: bool,
    #[arg(
        long = "priority",
        value_parser = parse_priority_override,
        help = "Specify per-file priority overrides as index=priority"
    )]
    priorities: Vec<FilePriorityOverrideArg>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ActionType {
    Pause,
    Resume,
    Remove,
    Reannounce,
    Recheck,
    Sequential,
    Rate,
}

#[derive(Args)]
struct TorrentActionArgs {
    #[arg(help = "Torrent identifier")]
    id: Uuid,
    #[arg(value_enum)]
    action: ActionType,
    #[arg(long, help = "Delete data when removing a torrent")]
    delete_data: bool,
    #[arg(long, help = "Enable sequential download when action=sequential")]
    enable: Option<bool>,
    #[arg(long, help = "Per-torrent download cap (bps) when action=rate")]
    download: Option<u64>,
    #[arg(long, help = "Per-torrent upload cap (bps) when action=rate")]
    upload: Option<u64>,
}

#[derive(Args, Default)]
struct TailArgs {
    #[arg(long, value_delimiter = ',', help = "Filter to torrent IDs")]
    torrent: Vec<Uuid>,
    #[arg(long, value_delimiter = ',', help = "Filter to event kinds")]
    event: Vec<String>,
    #[arg(long, value_delimiter = ',', help = "Filter to state names")]
    state: Vec<String>,
    #[arg(long, help = "Persist Last-Event-ID to this file")]
    resume_file: Option<PathBuf>,
    #[arg(
        long,
        default_value_t = 5,
        help = "Seconds to wait before reconnecting"
    )]
    retry_secs: u64,
}

#[derive(Copy, Clone, Debug, Default, ValueEnum)]
enum OutputFormat {
    #[default]
    Table,
    Json,
}

struct AppContext {
    client: Client,
    base_url: Url,
    api_key: Option<ApiKeyCredential>,
}

#[derive(Debug, Clone)]
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

    const fn exit_code(&self) -> i32 {
        match self {
            Self::Validation(_) => 2,
            Self::Failure(_) => 3,
        }
    }

    fn display_message(&self) -> String {
        match self {
            Self::Validation(message) => message.clone(),
            Self::Failure(error) => format!("{error:#}"),
        }
    }
}

#[derive(Clone)]
struct TelemetryEmitter {
    client: Client,
    endpoint: Url,
}

impl TelemetryEmitter {
    fn from_env() -> Option<Self> {
        let endpoint = env::var("REVAER_TELEMETRY_ENDPOINT").ok()?;
        let endpoint = endpoint.parse().ok()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .ok()?;
        Some(Self { client, endpoint })
    }

    async fn emit(
        &self,
        trace_id: &str,
        command: &str,
        outcome: &str,
        exit_code: i32,
        message: Option<&str>,
    ) {
        let event = TelemetryEvent {
            command,
            outcome,
            trace_id,
            exit_code,
            message,
            timestamp_ms: timestamp_now_ms(),
        };

        let _ = self
            .client
            .post(self.endpoint.clone())
            .json(&event)
            .send()
            .await;
    }
}

#[derive(Serialize)]
struct TelemetryEvent<'a> {
    command: &'a str,
    outcome: &'a str,
    trace_id: &'a str,
    exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<&'a str>,
    timestamp_ms: u64,
}

fn timestamp_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

const fn command_label(command: &Command) -> &'static str {
    match command {
        Command::Setup(SetupCommand::Start(_)) => "setup_start",
        Command::Setup(SetupCommand::Complete(_)) => "setup_complete",
        Command::Config(ConfigCommand::Get(_)) => "config_get",
        Command::Config(ConfigCommand::Set(_)) => "config_set",
        Command::Settings(SettingsCommand::Patch(_)) => "settings_patch",
        Command::Torrent(TorrentCommand::Add(_)) => "torrent_add",
        Command::Torrent(TorrentCommand::Remove(_)) => "torrent_remove",
        Command::Ls(_) => "ls",
        Command::Status(_) => "status",
        Command::Select(_) => "select",
        Command::Action(args) => match args.action {
            ActionType::Pause => "action_pause",
            ActionType::Resume => "action_resume",
            ActionType::Remove => "action_remove",
            ActionType::Reannounce => "action_reannounce",
            ActionType::Recheck => "action_recheck",
            ActionType::Sequential => "action_sequential",
            ActionType::Rate => "action_rate",
        },
        Command::Tail(_) => "tail",
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
    let source = args.source.trim();
    if source.is_empty() {
        return Err(CliError::validation("source must not be empty"));
    }

    let mut request = TorrentCreateRequest {
        id,
        magnet: None,
        metainfo: None,
        name: args.name,
        download_dir: None,
        sequential: None,
        include: Vec::new(),
        exclude: Vec::new(),
        skip_fluff: false,
        tags: Vec::new(),
        trackers: Vec::new(),
        max_download_bps: None,
        max_upload_bps: None,
    };

    if source.starts_with("magnet:") {
        request.magnet = Some(source.to_string());
    } else {
        let path = Path::new(source);
        let bytes = fs::read(path).map_err(|err| {
            CliError::failure(anyhow!(
                "failed to read torrent file '{}': {err}",
                path.display()
            ))
        })?;
        request.metainfo = Some(general_purpose::STANDARD.encode(&bytes));
        if request.name.is_none() {
            request.name = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string);
        }
    }

    let url = ctx
        .base_url
        .join("/v1/torrents")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| CliError::failure(anyhow!("request to /v1/torrents failed: {err}")))?;

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
        .join(&format!("/v1/torrents/{id}/action"))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&ApiTorrentAction::Remove { delete_data: false })
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!("request to /v1/torrents/{id}/action failed: {err}"))
        })?;

    if response.status().is_success() {
        println!("Torrent removal requested (id: {id})");
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

async fn handle_torrent_list(
    ctx: &AppContext,
    args: TorrentListArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let mut url = ctx
        .base_url
        .join("/v1/torrents")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    {
        let mut pairs = url.query_pairs_mut();
        if let Some(limit) = args.limit {
            pairs.append_pair("limit", &limit.to_string());
        }
        if let Some(cursor) = &args.cursor {
            pairs.append_pair("cursor", cursor);
        }
        if let Some(state) = &args.state {
            pairs.append_pair("state", state);
        }
        if let Some(tracker) = &args.tracker {
            pairs.append_pair("tracker", tracker);
        }
        if let Some(extension) = &args.extension {
            pairs.append_pair("extension", extension);
        }
        if let Some(tags) = &args.tags {
            pairs.append_pair("tags", tags);
        }
        if let Some(name) = &args.name {
            pairs.append_pair("name", name);
        }
    }

    let response = ctx
        .client
        .get(url)
        .send()
        .await
        .map_err(|err| CliError::failure(anyhow!("request to /v1/torrents failed: {err}")))?;

    if response.status().is_success() {
        let list = response
            .json::<TorrentListResponse>()
            .await
            .map_err(|err| CliError::failure(anyhow!("failed to parse torrent list: {err}")))?;
        render_torrent_list(&list, output)?;
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

async fn handle_torrent_status(
    ctx: &AppContext,
    args: TorrentStatusArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let url = ctx
        .base_url
        .join(&format!("/v1/torrents/{}", args.id))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;
    let response = ctx.client.get(url.as_ref()).send().await.map_err(|err| {
        CliError::failure(anyhow!("request to /v1/torrents/{{id}} failed: {err}"))
    })?;

    if response.status().is_success() {
        let detail = response
            .json::<TorrentDetail>()
            .await
            .map_err(|err| CliError::failure(anyhow!("failed to parse torrent detail: {err}")))?;
        render_torrent_detail(&detail, output)?;
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

async fn handle_torrent_select(ctx: &AppContext, args: TorrentSelectArgs) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let mut request = TorrentSelectionRequest {
        include: args.include.clone(),
        exclude: args.exclude.clone(),
        skip_fluff: Some(args.skip_fluff),
        priorities: Vec::new(),
    };
    for entry in &args.priorities {
        request.priorities.push(FilePriorityOverride {
            index: entry.index,
            priority: entry.priority,
        });
    }

    let url = ctx
        .base_url
        .join(&format!("/v1/torrents/{}/select", args.id))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/torrents/{{id}}/select failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        println!("Selection update accepted (id: {})", args.id);
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

async fn handle_torrent_action(ctx: &AppContext, args: TorrentActionArgs) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let action_payload = build_action_payload(&args)?;

    let url = ctx
        .base_url
        .join(&format!("/v1/torrents/{}/action", args.id))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&action_payload)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/torrents/{{id}}/action failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        println!("Action dispatched (id: {})", args.id);
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

async fn handle_tail(ctx: &AppContext, args: TailArgs) -> CliResult<()> {
    let mut resume_id = args
        .resume_file
        .as_ref()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|value| value.trim().parse::<u64>().ok());

    loop {
        let mut url = ctx
            .base_url
            .join("/v1/torrents/events")
            .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

        {
            let mut pairs = url.query_pairs_mut();
            if !args.torrent.is_empty() {
                let value = args
                    .torrent
                    .iter()
                    .map(Uuid::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                pairs.append_pair("torrent", &value);
            }
            if !args.event.is_empty() {
                let value = args.event.join(",");
                pairs.append_pair("event", &value);
            }
            if !args.state.is_empty() {
                let value = args.state.join(",");
                pairs.append_pair("state", &value);
            }
        }

        let builder = ctx.client.get(url);
        let builder = if let Some(id) = resume_id {
            builder.header(HEADER_LAST_EVENT_ID, id.to_string())
        } else {
            builder
        };

        let response = match builder.send().await {
            Ok(resp) => resp,
            Err(err) => {
                eprintln!(
                    "stream connection failed: {err:?}. retrying in {}s",
                    args.retry_secs
                );
                sleep(Duration::from_secs(args.retry_secs)).await;
                continue;
            }
        };

        if !response.status().is_success() {
            return Err(classify_problem(response).await);
        }

        match stream_events(response, &args, resume_id.as_mut()).await {
            Ok(last_id) => resume_id = last_id,
            Err(err) => {
                eprintln!("stream error: {err:?}. retrying in {}s", args.retry_secs);
                sleep(Duration::from_secs(args.retry_secs)).await;
            }
        }
    }
}

fn render_torrent_list(list: &TorrentListResponse, format: OutputFormat) -> CliResult<()> {
    match format {
        OutputFormat::Json => {
            let text = serde_json::to_string_pretty(list)
                .map_err(|err| CliError::failure(anyhow!("failed to format JSON: {err}")))?;
            println!("{text}");
        }
        OutputFormat::Table => {
            println!("{:<36} {:<18} {:>7} NAME", "ID", "STATE", "PROG");
            for summary in &list.torrents {
                let progress = format!("{:.1}%", summary.progress.percent_complete);
                let name = summary.name.as_deref().unwrap_or("<unnamed>");
                println!(
                    "{:<36} {:<18} {:>7} {}",
                    summary.id,
                    state_to_str(summary.state.kind),
                    progress,
                    name
                );
            }
            if let Some(next) = &list.next {
                println!("next cursor: {next}");
            }
        }
    }
    Ok(())
}

fn render_torrent_detail(detail: &TorrentDetail, format: OutputFormat) -> CliResult<()> {
    match format {
        OutputFormat::Json => {
            let text = serde_json::to_string_pretty(detail)
                .map_err(|err| CliError::failure(anyhow!("failed to format JSON: {err}")))?;
            println!("{text}");
        }
        OutputFormat::Table => {
            let summary = &detail.summary;
            println!("id: {}", summary.id);
            if let Some(name) = &summary.name {
                println!("name: {name}");
            }
            println!("state: {}", state_to_str(summary.state.kind));
            if let Some(message) = &summary.state.failure_message {
                println!("reason: {message}");
            }
            println!(
                "progress: {:.1}% ({}/{})",
                summary.progress.percent_complete,
                format_bytes(summary.progress.bytes_downloaded),
                format_bytes(summary.progress.bytes_total)
            );
            println!(
                "rates: down {} / up {}",
                format_bytes(summary.rates.download_bps),
                format_bytes(summary.rates.upload_bps)
            );
            if let Some(path) = &summary.library_path {
                println!("library: {path}");
            }
            if !summary.tags.is_empty() {
                println!("tags: {}", summary.tags.join(", "));
            }
            if !summary.trackers.is_empty() {
                println!("trackers: {}", summary.trackers.join(", "));
            }
            println!("sequential: {}", summary.sequential);
            println!("added: {}", summary.added_at);
            println!("updated: {}", summary.last_updated);
            if let Some(files) = &detail.files {
                println!("files:");
                println!(
                    "  {:>5} {:>12} {:>12} {:<8} path",
                    "index", "size", "done", "priority"
                );
                for file in files {
                    println!(
                        "  {:>5} {:>12} {:>12} {:<8} {}",
                        file.index,
                        format_bytes(file.size_bytes),
                        format_bytes(file.bytes_completed),
                        format_priority(file.priority),
                        file.path
                    );
                }
            }
        }
    }
    Ok(())
}

fn render_config_snapshot(snapshot: &ConfigSnapshot, format: OutputFormat) -> CliResult<()> {
    match format {
        OutputFormat::Json => {
            let text = serde_json::to_string_pretty(snapshot)
                .map_err(|err| CliError::failure(anyhow!("failed to format JSON: {err}")))?;
            println!("{text}");
        }
        OutputFormat::Table => {
            println!("revision: {}", snapshot.revision);
            println!("mode: {}", snapshot.app_profile.mode.as_str());
            println!("instance: {}", snapshot.app_profile.instance_name);
            println!(
                "http bind: {}:{}",
                snapshot.app_profile.bind_addr, snapshot.app_profile.http_port
            );
            println!(
                "engine: {} (listen port: {:?})",
                snapshot.engine_profile.implementation, snapshot.engine_profile.listen_port
            );
            println!("download root: {}", snapshot.engine_profile.download_root);
            println!("resume dir: {}", snapshot.engine_profile.resume_dir);
            println!("library root: {}", snapshot.fs_policy.library_root);
        }
    }
    Ok(())
}

const fn format_priority(priority: FilePriority) -> &'static str {
    match priority {
        FilePriority::Skip => "skip",
        FilePriority::Low => "low",
        FilePriority::Normal => "normal",
        FilePriority::High => "high",
    }
}

const fn state_to_str(kind: TorrentStateKind) -> &'static str {
    match kind {
        TorrentStateKind::Queued => "queued",
        TorrentStateKind::FetchingMetadata => "fetching_metadata",
        TorrentStateKind::Downloading => "downloading",
        TorrentStateKind::Seeding => "seeding",
        TorrentStateKind::Completed => "completed",
        TorrentStateKind::Failed => "failed",
        TorrentStateKind::Stopped => "stopped",
    }
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let value = bytes_to_f64(bytes);
    if value >= GIB {
        format!("{:.2} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.2} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.2} KiB", value / KIB)
    } else {
        format!("{bytes} B")
    }
}

const fn bytes_to_f64(value: u64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "u64 to f64 conversion is required for human-readable byte formatting"
    )]
    {
        value as f64
    }
}

fn build_action_payload(args: &TorrentActionArgs) -> CliResult<ApiTorrentAction> {
    let action = match args.action {
        ActionType::Pause => ApiTorrentAction::Pause,
        ActionType::Resume => ApiTorrentAction::Resume,
        ActionType::Remove => ApiTorrentAction::Remove {
            delete_data: args.delete_data,
        },
        ActionType::Reannounce => ApiTorrentAction::Reannounce,
        ActionType::Recheck => ApiTorrentAction::Recheck,
        ActionType::Sequential => {
            let enable = args.enable.ok_or_else(|| {
                CliError::validation("--enable must be provided for sequential action")
            })?;
            ApiTorrentAction::Sequential { enable }
        }
        ActionType::Rate => {
            if args.download.is_none() && args.upload.is_none() {
                return Err(CliError::validation(
                    "provide --download and/or --upload when action=rate",
                ));
            }
            ApiTorrentAction::Rate {
                download_bps: args.download,
                upload_bps: args.upload,
            }
        }
    };
    Ok(action)
}

fn parse_priority_override(value: &str) -> Result<FilePriorityOverrideArg, String> {
    let (index_str, priority_str) = value
        .split_once('=')
        .ok_or_else(|| "expected format index=priority".to_string())?;
    let index = index_str
        .trim()
        .parse::<u32>()
        .map_err(|_| "index must be an integer".to_string())?;
    let priority = match priority_str.trim().to_ascii_lowercase().as_str() {
        "skip" => FilePriority::Skip,
        "low" => FilePriority::Low,
        "normal" => FilePriority::Normal,
        "high" => FilePriority::High,
        other => return Err(format!("unknown priority '{other}'")),
    };
    Ok(FilePriorityOverrideArg { index, priority })
}

async fn stream_events(
    response: reqwest::Response,
    args: &TailArgs,
    mut resume_slot: Option<&mut u64>,
) -> CliResult<Option<u64>> {
    use futures_util::StreamExt;

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut current_event_id: Option<u64> = None;
    let mut current_data = Vec::new();
    let mut last_seen = resume_slot.as_ref().map(|slot| **slot);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|err| CliError::failure(anyhow!("failed to read event stream: {err}")))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim_end_matches('\r').to_string();
            buffer.drain(..=pos);
            if line.is_empty() {
                if current_data.is_empty() {
                    current_event_id = None;
                    continue;
                }
                let payload = current_data.join("\n");
                current_data.clear();
                if let Some(id) = current_event_id.take() {
                    if Some(id) == last_seen {
                        continue;
                    }
                    last_seen = Some(id);
                    if let Some(slot) = resume_slot.as_mut() {
                        **slot = id;
                    }
                    if let Some(path) = &args.resume_file {
                        let _ = fs::write(path, id.to_string());
                    }
                }
                match serde_json::from_str::<EventEnvelope>(&payload) {
                    Ok(event) => {
                        let text = serde_json::to_string_pretty(&event).map_err(|err| {
                            CliError::failure(anyhow!("failed to format event JSON: {err}"))
                        })?;
                        println!("{text}");
                    }
                    Err(err) => {
                        eprintln!("discarding malformed event payload: {err} -- {payload}");
                    }
                }
            } else if let Some(data) = line.strip_prefix("data:") {
                current_data.push(data.trim_start().to_string());
            } else if let Some(id) = line.strip_prefix("id:")
                && let Ok(value) = id.trim_start().parse::<u64>()
            {
                current_event_id = Some(value);
            }
        }
    }

    Ok(last_seen)
}

async fn handle_config_get(ctx: &AppContext, format: OutputFormat) -> CliResult<()> {
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

async fn handle_config_set(ctx: &AppContext, args: ConfigSetArgs) -> CliResult<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use httpmock::Method::PATCH;
    use httpmock::prelude::*;
    use reqwest::Client;
    use revaer_api::models::{
        TorrentDetail, TorrentFileView, TorrentListResponse, TorrentProgressView, TorrentRatesView,
        TorrentStateKind, TorrentStateView, TorrentSummary,
    };
    use revaer_config::{AppMode, AppProfile, ConfigSnapshot, EngineProfile, FsPolicy};
    use revaer_events::{Event, EventEnvelope};
    use serde_json::{Value, json};
    use std::{
        ffi::OsString,
        fs,
        io::Write,
        path::{Path, PathBuf},
    };
    use tokio::time::{Duration, timeout};
    use uuid::Uuid;

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

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        let mut file = fs::File::create(path).expect("create file");
        file.write_all(contents.as_bytes()).expect("write file");
    }

    fn temp_path(name: &str) -> PathBuf {
        let mut path = env::temp_dir();
        path.push(format!(
            "revaer-cli-test-{}-{}-{name}",
            std::process::id(),
            Uuid::new_v4()
        ));
        path
    }

    fn sample_snapshot() -> ConfigSnapshot {
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
            engine_profile: EngineProfile {
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
            },
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

    fn sample_summary(id: Uuid, now: chrono::DateTime<Utc>) -> TorrentSummary {
        TorrentSummary {
            id,
            name: Some("Example".into()),
            state: TorrentStateView {
                kind: TorrentStateKind::Downloading,
                failure_message: None,
            },
            progress: TorrentProgressView {
                bytes_downloaded: 1_024,
                bytes_total: 2_048,
                percent_complete: 50.0,
                eta_seconds: None,
            },
            rates: TorrentRatesView {
                download_bps: 1_024,
                upload_bps: 256,
                ratio: 0.5,
            },
            library_path: Some("/library/example".into()),
            download_dir: Some("/downloads/example".into()),
            sequential: false,
            tags: vec!["tag1".into()],
            trackers: vec!["https://tracker.example/announce".into()],
            added_at: now,
            completed_at: None,
            last_updated: now,
        }
    }

    #[test]
    fn parse_url_rejects_invalid_input() {
        let err = parse_url("not-a-url").expect_err("invalid URL should fail");
        assert!(err.contains("invalid URL"));
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
        let file_path = temp_path("settings.json");
        write_file(
            &file_path,
            r#"{
                "app_profile": { "instance_name": "custom" },
                "api_keys": [],
                "secrets": []
            }"#,
        );

        handle_config_set(
            &ctx,
            ConfigSetArgs {
                file: file_path.clone(),
            },
        )
        .await
        .expect("settings patch should succeed");
        mock.assert();
        let _ = fs::remove_file(file_path);
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

    #[tokio::test]
    async fn torrent_add_issues_post_request() {
        let server = MockServer::start_async().await;
        let id = Uuid::new_v4();
        let magnet = "magnet:?xt=urn:btih:demo";
        let name = "demo.torrent";

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/torrents")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "id": id,
                    "magnet": magnet,
                    "metainfo": null,
                    "name": name,
                    "download_dir": null,
                    "sequential": null,
                    "include": [],
                    "exclude": [],
                    "skip_fluff": false,
                    "tags": [],
                    "trackers": [],
                    "max_download_bps": null,
                    "max_upload_bps": null
                }));
            then.status(202);
        });

        let ctx = context_with_key(&server);
        let args = TorrentAddArgs {
            source: magnet.to_string(),
            name: Some(name.to_string()),
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

        let path = format!("/v1/torrents/{id}/action");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "type": "remove",
                    "delete_data": false
                }));
            then.status(202);
        });

        let ctx = context_with_key(&server);
        let args = TorrentRemoveArgs { id };

        handle_torrent_remove(&ctx, args)
            .await
            .expect("torrent remove should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn torrent_list_renders_table() {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let now = Utc::now();
        let list = TorrentListResponse {
            torrents: vec![sample_summary(torrent_id, now)],
            next: Some("cursor-1".into()),
        };
        server.mock(move |when, then| {
            when.method(GET).path("/v1/torrents");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!(list));
        });

        let ctx = context_with(&server, None);
        handle_torrent_list(&ctx, TorrentListArgs::default(), OutputFormat::Table)
            .await
            .expect("list should succeed");
    }

    #[tokio::test]
    async fn torrent_status_renders_detail() {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let now = Utc::now();
        let detail = TorrentDetail {
            summary: sample_summary(torrent_id, now),
            files: Some(vec![TorrentFileView {
                index: 0,
                path: "example.mkv".into(),
                size_bytes: 2_048,
                bytes_completed: 1_024,
                priority: FilePriority::High,
                selected: true,
            }]),
        };
        server.mock(move |when, then| {
            when.method(GET)
                .path(format!("/v1/torrents/{torrent_id}").as_str());
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!(detail));
        });

        let ctx = context_with(&server, None);
        handle_torrent_status(
            &ctx,
            TorrentStatusArgs { id: torrent_id },
            OutputFormat::Table,
        )
        .await
        .expect("status should succeed");
    }

    #[tokio::test]
    async fn torrent_select_sends_priorities() {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let path = format!("/v1/torrents/{torrent_id}/select");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret");
            then.status(200);
        });

        let ctx = context_with_key(&server);
        let args = TorrentSelectArgs {
            id: torrent_id,
            include: vec!["**/*.mkv".into()],
            exclude: Vec::new(),
            skip_fluff: true,
            priorities: vec![FilePriorityOverrideArg {
                index: 7,
                priority: FilePriority::High,
            }],
        };
        handle_torrent_select(&ctx, args)
            .await
            .expect("select should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn torrent_action_sequential_with_enable() {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let path = format!("/v1/torrents/{torrent_id}/action");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "type": "sequential",
                    "enable": true
                }));
            then.status(202);
        });

        let ctx = context_with_key(&server);
        let args = TorrentActionArgs {
            id: torrent_id,
            action: ActionType::Sequential,
            enable: Some(true),
            delete_data: false,
            download: None,
            upload: None,
        };

        handle_torrent_action(&ctx, args)
            .await
            .expect("action should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn torrent_action_rate_includes_caps() {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let path = format!("/v1/torrents/{torrent_id}/action");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "type": "rate",
                    "download_bps": 2048,
                    "upload_bps": 1024
                }));
            then.status(202);
        });

        let ctx = context_with_key(&server);
        let args = TorrentActionArgs {
            id: torrent_id,
            action: ActionType::Rate,
            enable: None,
            delete_data: false,
            download: Some(2048),
            upload: Some(1024),
        };

        handle_torrent_action(&ctx, args)
            .await
            .expect("rate action should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn stream_events_updates_resume_marker() {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let event = EventEnvelope {
            id: 1,
            timestamp: Utc::now(),
            event: Event::TorrentAdded {
                torrent_id,
                name: "demo".into(),
            },
        };
        let payload = serde_json::to_string(&event).expect("event JSON");
        server.mock(move |when, then| {
            when.method(GET).path("/v1/torrents/events");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(format!("id:1\ndata:{payload}\n\n"));
        });

        let ctx = context_with_key(&server);
        let resume_file = temp_path("resume.txt");
        let mut resume_slot = 0_u64;
        let args = TailArgs {
            torrent: Vec::new(),
            event: Vec::new(),
            state: Vec::new(),
            resume_file: Some(resume_file.clone()),
            retry_secs: 0,
        };

        let response = ctx
            .client
            .get(ctx.base_url.join("/v1/torrents/events").unwrap())
            .send()
            .await
            .expect("send request");
        let last = stream_events(response, &args, Some(&mut resume_slot))
            .await
            .expect("stream should succeed");
        assert_eq!(last, Some(1));
        assert_eq!(resume_slot, 1);
        let saved = fs::read_to_string(resume_file).expect("resume file");
        assert_eq!(saved.trim(), "1");
    }

    #[tokio::test]
    async fn stream_events_discards_malformed_payloads() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(GET).path("/v1/torrents/events");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body("id:2\ndata:{\"bad\":true}\n\n");
        });

        let ctx = context_with_key(&server);
        let args = TailArgs {
            torrent: Vec::new(),
            event: Vec::new(),
            state: Vec::new(),
            resume_file: None,
            retry_secs: 0,
        };
        let response = ctx
            .client
            .get(ctx.base_url.join("/v1/torrents/events").unwrap())
            .send()
            .await
            .expect("send request");
        let id = stream_events(response, &args, None)
            .await
            .expect("stream should succeed");
        assert_eq!(id, Some(2));
    }

    #[tokio::test]
    async fn handle_tail_writes_resume_file() {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let event = EventEnvelope {
            id: 3,
            timestamp: Utc::now(),
            event: Event::TorrentRemoved { torrent_id },
        };
        let payload = serde_json::to_string(&event).expect("event JSON");
        server.mock(move |when, then| {
            when.method(GET).path("/v1/torrents/events");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(format!("id:3\ndata:{payload}\n\n"));
        });

        let ctx = context_with_key(&server);
        let resume_path = temp_path("tail.txt");
        let args = TailArgs {
            torrent: Vec::new(),
            event: Vec::new(),
            state: Vec::new(),
            resume_file: Some(resume_path.clone()),
            retry_secs: 0,
        };

        let result = timeout(Duration::from_millis(200), handle_tail(&ctx, args)).await;
        assert!(
            result.is_err(),
            "tail should keep running and be cancelled by timeout"
        );
        let saved = fs::read_to_string(resume_path).expect("resume file");
        assert_eq!(saved.trim(), "3");
    }

    #[tokio::test]
    async fn telemetry_emitter_emits_event() {
        let server = MockServer::start_async().await;
        let mock = server.mock(|when, then| {
            when.method(POST).path("/telemetry");
            then.status(200);
        });

        let emitter = TelemetryEmitter {
            client: Client::new(),
            endpoint: format!("{}/telemetry", server.base_url())
                .parse()
                .expect("valid URL"),
        };

        emitter
            .emit("trace", "command", "success", 0, Some("message"))
            .await;

        mock.assert();
    }

    #[test]
    fn parse_api_key_requires_secret() {
        let err = parse_api_key(Some("key_only:".to_string()))
            .expect_err("expected missing secret to fail");
        assert!(
            matches!(err, CliError::Validation(message) if message.contains("cannot be empty"))
        );
    }

    #[test]
    fn parse_api_key_accepts_valid_pair() -> CliResult<()> {
        let parsed = parse_api_key(Some("alpha:bravo".to_string()))?.expect("expected credentials");
        assert_eq!(parsed.key_id, "alpha");
        assert_eq!(parsed.secret, "bravo");
        Ok(())
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

    #[test]
    fn random_string_produces_expected_length() {
        let generated = random_string(16);
        assert_eq!(generated.len(), 16);
        assert!(generated.chars().all(|ch| ch.is_ascii_alphanumeric()));
    }

    #[test]
    fn format_bytes_displays_expected_units() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.00 KiB");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.00 MiB");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.00 GiB");
    }

    #[test]
    fn state_to_str_maps_variants() {
        assert_eq!(state_to_str(TorrentStateKind::Queued), "queued");
        assert_eq!(state_to_str(TorrentStateKind::Completed), "completed");
    }

    #[test]
    fn format_priority_labels_variants() {
        assert_eq!(format_priority(FilePriority::Skip), "skip");
        assert_eq!(format_priority(FilePriority::High), "high");
    }

    #[test]
    fn build_action_payload_requires_enable_flag() {
        let args = TorrentActionArgs {
            id: Uuid::new_v4(),
            action: ActionType::Sequential,
            enable: None,
            delete_data: false,
            download: None,
            upload: None,
        };
        let err = build_action_payload(&args).expect_err("missing enable should fail");
        assert!(matches!(err, CliError::Validation(message) if message.contains("--enable")));
    }

    #[test]
    fn build_action_payload_validates_rate_limits() {
        let args = TorrentActionArgs {
            id: Uuid::new_v4(),
            action: ActionType::Rate,
            enable: None,
            delete_data: false,
            download: None,
            upload: None,
        };
        let err =
            build_action_payload(&args).expect_err("missing rate values should fail validation");
        assert!(matches!(err, CliError::Validation(message) if message.contains("download")));
    }

    #[test]
    fn build_action_payload_rate_accepts_partial_limits() -> CliResult<()> {
        let args = TorrentActionArgs {
            id: Uuid::new_v4(),
            action: ActionType::Rate,
            enable: None,
            delete_data: false,
            download: Some(1024),
            upload: None,
        };
        match build_action_payload(&args)? {
            ApiTorrentAction::Rate {
                download_bps,
                upload_bps,
            } => {
                assert_eq!(download_bps, Some(1024));
                assert_eq!(upload_bps, None);
            }
            other => panic!("unexpected payload {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn parse_priority_override_rejects_invalid_payload() {
        let err = parse_priority_override("abc=skip").expect_err("invalid index should fail");
        assert!(err.contains("index"));
        let err = parse_priority_override("10=unknown").expect_err("invalid priority");
        assert!(err.contains("unknown priority"));
    }

    #[test]
    fn parse_priority_override_accepts_values() {
        let parsed = parse_priority_override("42=high").expect("valid override");
        assert_eq!(parsed.index, 42);
        assert_eq!(parsed.priority, FilePriority::High);
    }

    #[test]
    fn command_label_matches_variants() {
        assert_eq!(
            command_label(&Command::Torrent(TorrentCommand::Add(TorrentAddArgs {
                source: "magnet:?xt=urn:btih:demo".to_string(),
                name: None,
                id: None,
            }))),
            "torrent_add"
        );
        assert_eq!(
            command_label(&Command::Action(TorrentActionArgs {
                id: Uuid::nil(),
                action: ActionType::Pause,
                enable: None,
                delete_data: false,
                download: None,
                upload: None,
            })),
            "action_pause"
        );
    }

    #[test]
    fn timestamp_now_ms_returns_positive_value() {
        assert!(timestamp_now_ms() > 0);
    }

    #[cfg(target_family = "unix")]
    #[test]
    fn path_to_string_rejects_non_utf8() {
        use std::os::unix::ffi::OsStringExt;
        let path = PathBuf::from(OsString::from_vec(vec![0xFF]));
        let err = path_to_string(&path).expect_err("invalid UTF-8 should fail");
        assert!(matches!(err, CliError::Validation(_)));
    }
}
