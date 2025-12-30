//! Command-line client for interacting with a Revaer server instance.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use reqwest::Url;
use uuid::Uuid;

use crate::client::{AppContext, CliDependencies, CliResult, parse_api_key, parse_url};
use crate::commands::torrents::{FilePriorityOverrideArg, StorageModeArg};
use crate::commands::{config, setup, tail, torrents};

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

    let api_key = match parse_api_key(cli.api_key.clone()) {
        Ok(key) => key,
        Err(err) => {
            eprintln!("error: {}", err.display_message());
            return err.exit_code();
        }
    };
    let ctx = AppContext {
        client: deps.client.clone(),
        base_url: cli.api_url.clone(),
        api_key,
    };

    let result = dispatch(cli, &ctx).await;

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

async fn dispatch(cli: Cli, deps: &AppContext) -> CliResult<()> {
    match cli.command {
        Command::Setup(setup_command) => match setup_command {
            SetupCommand::Start(args) => setup::handle_setup_start(deps, args).await,
            SetupCommand::Complete(args) => setup::handle_setup_complete(deps, args).await,
        },
        Command::Config(config_command) => match config_command {
            ConfigCommand::Get(_) => config::handle_config_get(deps, cli.output).await,
            ConfigCommand::Set(args) => config::handle_config_set(deps, args).await,
        },
        Command::Settings(settings_command) => match settings_command {
            SettingsCommand::Patch(args) => config::handle_config_set(deps, args).await,
        },
        Command::Torrent(torrent_command) => match torrent_command {
            TorrentCommand::Add(args) => torrents::handle_torrent_add(deps, args).await,
            TorrentCommand::Remove(args) => torrents::handle_torrent_remove(deps, args).await,
        },
        Command::Ls(args) => torrents::handle_torrent_list(deps, args, cli.output).await,
        Command::Status(args) => torrents::handle_torrent_status(deps, args, cli.output).await,
        Command::Select(args) => torrents::handle_torrent_select(deps, args).await,
        Command::Action(args) => torrents::handle_torrent_action(deps, args).await,
        Command::Tail(args) => tail::handle_tail(deps, args).await,
    }
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
            ActionType::Move => "action_move",
        },
        Command::Tail(_) => "tail",
    }
}

#[derive(Parser)]
#[command(name = "revaer", about = "Administrative CLI for the Revaer platform")]
pub(crate) struct Cli {
    #[arg(
        long,
        global = true,
        env = "REVAER_API_URL",
        value_parser = parse_url,
        default_value = "http://127.0.0.1:7070"
    )]
    pub api_url: Url,
    #[arg(long, global = true, env = "REVAER_API_KEY")]
    pub api_key: Option<String>,
    #[arg(
        long,
        global = true,
        env = "REVAER_HTTP_TIMEOUT_SECS",
        default_value_t = 10
    )]
    pub timeout: u64,
    #[arg(
        long = "output",
        alias = "format",
        global = true,
        value_enum,
        default_value_t = OutputFormat::Table,
        help = "Select output format for commands that render structured data"
    )]
    pub output: OutputFormat,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
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
pub(crate) enum SetupCommand {
    Start(SetupStartArgs),
    Complete(SetupCompleteArgs),
}

#[derive(Subcommand)]
pub(crate) enum ConfigCommand {
    Get(ConfigGetArgs),
    Set(ConfigSetArgs),
}

#[derive(Subcommand)]
pub(crate) enum SettingsCommand {
    Patch(ConfigSetArgs),
}

#[derive(Subcommand)]
pub(crate) enum TorrentCommand {
    Add(TorrentAddArgs),
    Remove(TorrentRemoveArgs),
}

#[derive(Args)]
pub(crate) struct SetupStartArgs {
    #[arg(long, help = "Optional issuer identity to record with the token")]
    pub issued_by: Option<String>,
    #[arg(
        long,
        help = "Optional TTL for the token, defaults to server-side configuration"
    )]
    pub ttl_seconds: Option<u64>,
}

#[derive(Args)]
pub(crate) struct SetupCompleteArgs {
    #[arg(long, env = "REVAER_SETUP_TOKEN")]
    pub token: Option<String>,
    #[arg(long)]
    pub instance: String,
    #[arg(long, default_value = "127.0.0.1")]
    pub bind: String,
    #[arg(long, default_value_t = 7070)]
    pub port: u16,
    #[arg(long, value_parser = parse_existing_directory)]
    pub resume_dir: PathBuf,
    #[arg(long, value_parser = parse_existing_directory)]
    pub download_root: PathBuf,
    #[arg(long, value_parser = parse_existing_directory)]
    pub library_root: PathBuf,
    #[arg(long)]
    pub api_key_label: String,
    #[arg(long, help = "Optional API key identifier override")]
    pub api_key_id: Option<String>,
    #[arg(
        long,
        help = "Passphrase for encrypting secrets; prompts interactively if omitted"
    )]
    pub passphrase: Option<String>,
}

#[derive(Args, Default)]
pub(crate) struct ConfigGetArgs {}

#[derive(Args, Clone)]
pub(crate) struct ConfigSetArgs {
    #[arg(long, value_parser = parse_existing_file, help = "Path to the JSON settings patch")]
    pub file: PathBuf,
}

#[derive(Args, Default)]
pub(crate) struct TorrentListArgs {
    #[arg(long)]
    pub limit: Option<u32>,
    #[arg(long)]
    pub cursor: Option<String>,
    #[arg(long)]
    pub state: Option<String>,
    #[arg(long)]
    pub tracker: Option<String>,
    #[arg(long)]
    pub extension: Option<String>,
    #[arg(long)]
    pub tags: Option<String>,
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Args)]
pub(crate) struct TorrentStatusArgs {
    #[arg(help = "Torrent identifier")]
    pub id: Uuid,
}

#[derive(Args)]
pub(crate) struct TorrentAddArgs {
    #[arg(long, help = "Magnet URI or .torrent file path")]
    pub source: String,
    #[arg(long, help = "Optional friendly name for the torrent")]
    pub name: Option<String>,
    #[arg(long, help = "Optional torrent ID override")]
    pub id: Option<Uuid>,
    #[arg(
        long,
        value_enum,
        help = "Storage allocation mode (sparse or allocate)"
    )]
    pub storage_mode: Option<StorageModeArg>,
}

#[derive(Args)]
pub(crate) struct TorrentRemoveArgs {
    #[arg(help = "Torrent identifier")]
    pub id: Uuid,
}

#[derive(Args, Default)]
pub(crate) struct TorrentSelectArgs {
    #[arg(help = "Torrent identifier")]
    pub id: Uuid,
    #[arg(
        long,
        value_delimiter = ',',
        help = "Glob-style patterns to force inclusion"
    )]
    pub include: Vec<String>,
    #[arg(
        long,
        value_delimiter = ',',
        help = "Glob-style patterns to force exclusion"
    )]
    pub exclude: Vec<String>,
    #[arg(long, default_value_t = false, help = "Skip fluff files by default")]
    pub skip_fluff: bool,
    #[arg(
        long,
        value_delimiter = ',',
        value_parser = crate::commands::torrents::parse_priority_override,
        help = "File priority overrides expressed as index=priority"
    )]
    pub priorities: Vec<FilePriorityOverrideArg>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub(crate) enum ActionType {
    Pause,
    Resume,
    Remove,
    Reannounce,
    Recheck,
    Sequential,
    Rate,
    Move,
}

#[derive(Args)]
pub(crate) struct TorrentActionArgs {
    #[arg(help = "Torrent identifier")]
    pub id: Uuid,
    #[arg(value_enum)]
    pub action: ActionType,
    #[arg(long, help = "Delete data when removing a torrent")]
    pub delete_data: bool,
    #[arg(long, help = "Enable sequential download when action=sequential")]
    pub enable: Option<bool>,
    #[arg(long, help = "Per-torrent download cap (bps) when action=rate")]
    pub download: Option<u64>,
    #[arg(long, help = "Per-torrent upload cap (bps) when action=rate")]
    pub upload: Option<u64>,
    #[arg(long, help = "Target download directory when action=move")]
    pub download_dir: Option<String>,
}

#[derive(Args, Default, Clone)]
pub(crate) struct TailArgs {
    #[arg(long, value_delimiter = ',', help = "Filter to torrent IDs")]
    pub torrent: Vec<Uuid>,
    #[arg(long, value_delimiter = ',', help = "Filter to event kinds")]
    pub event: Vec<String>,
    #[arg(long, value_delimiter = ',', help = "Filter to state names")]
    pub state: Vec<String>,
    #[arg(long, help = "Persist Last-Event-ID to this file")]
    pub resume_file: Option<PathBuf>,
    #[arg(
        long,
        default_value_t = 5,
        help = "Seconds to wait before reconnecting"
    )]
    pub retry_secs: u64,
}

#[derive(Copy, Clone, Debug, Default, ValueEnum)]
pub(crate) enum OutputFormat {
    #[default]
    Table,
    Json,
}

fn parse_existing_file(path: &str) -> Result<PathBuf, String> {
    let buf = PathBuf::from(path);
    if buf.is_file() {
        Ok(buf)
    } else {
        Err(format!("file '{path}' does not exist"))
    }
}

fn parse_existing_directory(path: &str) -> Result<PathBuf, String> {
    let buf = PathBuf::from(path);
    if buf.is_dir() {
        Ok(buf)
    } else {
        Err(format!("directory '{path}' does not exist"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{CliError, parse_api_key, parse_url, timestamp_now_ms};
    use anyhow::{Result, anyhow};

    #[test]
    fn parse_url_rejects_invalid_input() -> Result<()> {
        let err = parse_url("not-a-url")
            .err()
            .ok_or_else(|| anyhow!("expected invalid URL error"))?;
        assert!(err.contains("invalid URL"));
        Ok(())
    }

    #[test]
    fn parse_api_key_requires_secret() -> Result<()> {
        let err = parse_api_key(Some("key_only:".to_string()))
            .err()
            .ok_or_else(|| anyhow!("expected missing secret error"))?;
        assert!(
            matches!(err, CliError::Validation(message) if message.contains("cannot be empty"))
        );
        Ok(())
    }

    #[test]
    fn command_label_matches_variants() {
        assert_eq!(
            command_label(&Command::Torrent(TorrentCommand::Add(TorrentAddArgs {
                source: "magnet:?xt=urn:btih:demo".to_string(),
                name: None,
                id: None,
                storage_mode: None,
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
                download_dir: None,
            })),
            "action_pause"
        );
    }

    #[test]
    fn timestamp_now_ms_returns_positive_value() {
        assert!(timestamp_now_ms() > 0);
    }

    #[test]
    fn parse_existing_file_verifies_path() -> Result<()> {
        let tmp = std::env::temp_dir().join(format!("revaer-cli-{}.txt", Uuid::new_v4()));
        std::fs::write(&tmp, b"ok")?;
        let tmp_path = tmp.to_str().ok_or_else(|| anyhow!("invalid temp path"))?;
        assert!(parse_existing_file(tmp_path).is_ok());
        let missing =
            std::env::temp_dir().join(format!("revaer-cli-missing-{}.txt", Uuid::new_v4()));
        let missing_path = missing
            .to_str()
            .ok_or_else(|| anyhow!("invalid missing path"))?;
        assert!(parse_existing_file(missing_path).is_err());
        Ok(())
    }

    #[test]
    fn parse_existing_directory_verifies_path() -> Result<()> {
        let dir = std::env::temp_dir();
        let dir_path = dir.to_str().ok_or_else(|| anyhow!("invalid dir path"))?;
        assert!(parse_existing_directory(dir_path).is_ok());
        let missing = dir.join(format!("revaer-cli-dir-{}", Uuid::new_v4()));
        let missing_path = missing
            .to_str()
            .ok_or_else(|| anyhow!("invalid missing dir path"))?;
        assert!(parse_existing_directory(missing_path).is_err());
        Ok(())
    }
}
