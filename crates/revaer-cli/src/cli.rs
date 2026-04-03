//! Command-line client for interacting with a Revaer server instance.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use reqwest::Url;
use serde::Deserialize;
use uuid::Uuid;

use crate::client::{AppContext, CliDependencies, CliResult, parse_api_key, parse_url};
use crate::commands::indexers::{
    parse_import_job_id, parse_indexer_instance_id, parse_policy_rule_id, parse_policy_set_id,
    parse_routing_policy_id, parse_torznab_instance_id,
};
use crate::commands::torrents::{FilePriorityOverrideArg, StorageModeArg};
use crate::commands::{config, indexers, setup, tail, torrents};

/// Parses CLI arguments, executes the requested command, and handles
/// user-facing telemetry emission. Returns the process exit code.
pub async fn run() -> i32 {
    run_with_cli(Cli::parse()).await
}

async fn run_with_cli(cli: Cli) -> i32 {
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
        Command::Setup(setup_command) => dispatch_setup(setup_command, deps).await,
        Command::Config(config_command) => dispatch_config(config_command, deps, cli.output).await,
        Command::Settings(settings_command) => dispatch_settings(settings_command, deps).await,
        Command::Torrent(torrent_command) => dispatch_torrent(torrent_command, deps).await,
        Command::Indexer(indexer_command) => {
            dispatch_indexer(indexer_command, deps, cli.output).await
        }
        Command::Ls(args) => torrents::handle_torrent_list(deps, args, cli.output).await,
        Command::Status(args) => torrents::handle_torrent_status(deps, args, cli.output).await,
        Command::Select(args) => torrents::handle_torrent_select(deps, args).await,
        Command::Action(args) => torrents::handle_torrent_action(deps, args).await,
        Command::Tail(args) => tail::handle_tail(deps, args).await,
    }
}

async fn dispatch_setup(command: SetupCommand, deps: &AppContext) -> CliResult<()> {
    match command {
        SetupCommand::Start(args) => setup::handle_setup_start(deps, args).await,
        SetupCommand::Complete(args) => setup::handle_setup_complete(deps, args).await,
    }
}

async fn dispatch_config(
    command: ConfigCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        ConfigCommand::Get(_) => config::handle_config_get(deps, output).await,
        ConfigCommand::Set(args) => config::handle_config_set(deps, args).await,
    }
}

async fn dispatch_settings(command: SettingsCommand, deps: &AppContext) -> CliResult<()> {
    match command {
        SettingsCommand::Patch(args) => config::handle_config_set(deps, args).await,
    }
}

async fn dispatch_torrent(command: TorrentCommand, deps: &AppContext) -> CliResult<()> {
    match command {
        TorrentCommand::Add(args) => torrents::handle_torrent_add(deps, args).await,
        TorrentCommand::Remove(args) => torrents::handle_torrent_remove(deps, args).await,
    }
}

async fn dispatch_indexer(
    command: IndexerCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        IndexerCommand::Import(import_command) => {
            dispatch_indexer_import(import_command, deps, output).await
        }
        IndexerCommand::Torznab(torznab_command) => {
            dispatch_indexer_torznab(torznab_command, deps, output).await
        }
        IndexerCommand::Policy(policy_command) => {
            dispatch_indexer_policy(*policy_command, deps, output).await
        }
        IndexerCommand::Instance(instance_command) => {
            dispatch_indexer_instance(*instance_command, deps, output).await
        }
        IndexerCommand::Read(read_command) => {
            dispatch_indexer_read(*read_command, deps, output).await
        }
    }
}

async fn dispatch_indexer_import(
    command: IndexerImportCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        IndexerImportCommand::Create(args) => {
            indexers::handle_import_job_create(deps, args, output).await
        }
        IndexerImportCommand::RunProwlarrApi(args) => {
            indexers::handle_import_job_run_prowlarr_api(deps, args).await
        }
        IndexerImportCommand::RunProwlarrBackup(args) => {
            indexers::handle_import_job_run_prowlarr_backup(deps, args).await
        }
        IndexerImportCommand::Status(args) => {
            indexers::handle_import_job_status(deps, args, output).await
        }
        IndexerImportCommand::Results(args) => {
            indexers::handle_import_job_results(deps, args, output).await
        }
    }
}

async fn dispatch_indexer_torznab(
    command: TorznabCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        TorznabCommand::Create(args) => indexers::handle_torznab_create(deps, args, output).await,
        TorznabCommand::Rotate(args) => indexers::handle_torznab_rotate(deps, args, output).await,
        TorznabCommand::SetState(args) => indexers::handle_torznab_set_state(deps, args).await,
        TorznabCommand::Delete(args) => indexers::handle_torznab_delete(deps, args).await,
    }
}

async fn dispatch_indexer_policy(
    command: PolicyCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        PolicyCommand::SetCreate(args) => {
            indexers::handle_policy_set_create(deps, args, output).await
        }
        PolicyCommand::SetUpdate(args) => {
            indexers::handle_policy_set_update(deps, args, output).await
        }
        PolicyCommand::SetEnable(args) => indexers::handle_policy_set_enable(deps, args).await,
        PolicyCommand::SetDisable(args) => indexers::handle_policy_set_disable(deps, args).await,
        PolicyCommand::SetReorder(args) => indexers::handle_policy_set_reorder(deps, args).await,
        PolicyCommand::RuleCreate(args) => {
            indexers::handle_policy_rule_create(deps, *args, output).await
        }
        PolicyCommand::RuleEnable(args) => indexers::handle_policy_rule_enable(deps, args).await,
        PolicyCommand::RuleDisable(args) => indexers::handle_policy_rule_disable(deps, args).await,
        PolicyCommand::RuleReorder(args) => indexers::handle_policy_rule_reorder(deps, args).await,
    }
}

async fn dispatch_indexer_instance(
    command: IndexerInstanceCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        IndexerInstanceCommand::TestPrepare(args) => {
            indexers::handle_indexer_instance_test_prepare(deps, args, output).await
        }
        IndexerInstanceCommand::TestFinalize(args) => {
            indexers::handle_indexer_instance_test_finalize(deps, args, output).await
        }
    }
}

async fn dispatch_indexer_read(
    command: IndexerReadCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        IndexerReadCommand::Tags => indexers::handle_tag_list(deps, output).await,
        IndexerReadCommand::Secrets => indexers::handle_secret_list(deps, output).await,
        IndexerReadCommand::SearchProfiles => {
            indexers::handle_search_profile_list(deps, output).await
        }
        IndexerReadCommand::PolicySets => indexers::handle_policy_set_list(deps, output).await,
        IndexerReadCommand::RoutingPolicies => {
            indexers::handle_routing_policy_list(deps, output).await
        }
        IndexerReadCommand::RoutingPolicy(args) => {
            indexers::handle_routing_policy_read(deps, args, output).await
        }
        IndexerReadCommand::RateLimits => {
            indexers::handle_rate_limit_policy_list(deps, output).await
        }
        IndexerReadCommand::Instances => indexers::handle_indexer_instance_list(deps, output).await,
        IndexerReadCommand::TorznabInstances => {
            indexers::handle_torznab_instance_list(deps, output).await
        }
        IndexerReadCommand::BackupExport => {
            indexers::handle_indexer_backup_export(deps, output).await
        }
        IndexerReadCommand::Connectivity(args) => {
            indexers::handle_indexer_connectivity_read(deps, args, output).await
        }
        IndexerReadCommand::Reputation(args) => {
            indexers::handle_indexer_reputation_read(deps, args, output).await
        }
        IndexerReadCommand::HealthEvents(args) => {
            indexers::handle_indexer_health_events_read(deps, args, output).await
        }
        IndexerReadCommand::Rss(args) => {
            indexers::handle_indexer_rss_read(deps, args, output).await
        }
        IndexerReadCommand::RssItems(args) => {
            indexers::handle_indexer_rss_items_read(deps, args, output).await
        }
    }
}

fn command_label(command: &Command) -> &'static str {
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
        Command::Indexer(IndexerCommand::Import(IndexerImportCommand::Create(_))) => {
            "indexer_import_create"
        }
        Command::Indexer(IndexerCommand::Import(IndexerImportCommand::RunProwlarrApi(_))) => {
            "indexer_import_run_prowlarr_api"
        }
        Command::Indexer(IndexerCommand::Import(IndexerImportCommand::RunProwlarrBackup(_))) => {
            "indexer_import_run_prowlarr_backup"
        }
        Command::Indexer(IndexerCommand::Import(IndexerImportCommand::Status(_))) => {
            "indexer_import_status"
        }
        Command::Indexer(IndexerCommand::Import(IndexerImportCommand::Results(_))) => {
            "indexer_import_results"
        }
        Command::Indexer(IndexerCommand::Torznab(TorznabCommand::Create(_))) => {
            "indexer_torznab_create"
        }
        Command::Indexer(IndexerCommand::Torznab(TorznabCommand::Rotate(_))) => {
            "indexer_torznab_rotate"
        }
        Command::Indexer(IndexerCommand::Torznab(TorznabCommand::SetState(_))) => {
            "indexer_torznab_set_state"
        }
        Command::Indexer(IndexerCommand::Torznab(TorznabCommand::Delete(_))) => {
            "indexer_torznab_delete"
        }
        Command::Indexer(IndexerCommand::Policy(policy_command)) => match policy_command.as_ref() {
            PolicyCommand::SetCreate(_) => "indexer_policy_set_create",
            PolicyCommand::SetUpdate(_) => "indexer_policy_set_update",
            PolicyCommand::SetEnable(_) => "indexer_policy_set_enable",
            PolicyCommand::SetDisable(_) => "indexer_policy_set_disable",
            PolicyCommand::SetReorder(_) => "indexer_policy_set_reorder",
            PolicyCommand::RuleCreate(_) => "indexer_policy_rule_create",
            PolicyCommand::RuleEnable(_) => "indexer_policy_rule_enable",
            PolicyCommand::RuleDisable(_) => "indexer_policy_rule_disable",
            PolicyCommand::RuleReorder(_) => "indexer_policy_rule_reorder",
        },
        Command::Indexer(IndexerCommand::Instance(instance_command)) => {
            match instance_command.as_ref() {
                IndexerInstanceCommand::TestPrepare(_) => "indexer_instance_test_prepare",
                IndexerInstanceCommand::TestFinalize(_) => "indexer_instance_test_finalize",
            }
        }
        Command::Indexer(IndexerCommand::Read(read_command)) => match read_command.as_ref() {
            IndexerReadCommand::Tags => "indexer_read_tags",
            IndexerReadCommand::Secrets => "indexer_read_secrets",
            IndexerReadCommand::SearchProfiles => "indexer_read_search_profiles",
            IndexerReadCommand::PolicySets => "indexer_read_policy_sets",
            IndexerReadCommand::RoutingPolicies => "indexer_read_routing_policies",
            IndexerReadCommand::RoutingPolicy(_) => "indexer_read_routing_policy",
            IndexerReadCommand::RateLimits => "indexer_read_rate_limits",
            IndexerReadCommand::Instances => "indexer_read_instances",
            IndexerReadCommand::TorznabInstances => "indexer_read_torznab_instances",
            IndexerReadCommand::BackupExport => "indexer_read_backup_export",
            IndexerReadCommand::Connectivity(_) => "indexer_read_connectivity",
            IndexerReadCommand::Reputation(_) => "indexer_read_reputation",
            IndexerReadCommand::HealthEvents(_) => "indexer_read_health_events",
            IndexerReadCommand::Rss(_) => "indexer_read_rss",
            IndexerReadCommand::RssItems(_) => "indexer_read_rss_items",
        },
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
    #[command(subcommand)]
    Indexer(IndexerCommand),
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

#[derive(Subcommand)]
pub(crate) enum IndexerCommand {
    #[command(subcommand)]
    Import(IndexerImportCommand),
    #[command(subcommand)]
    Torznab(TorznabCommand),
    #[command(subcommand)]
    Policy(Box<PolicyCommand>),
    #[command(subcommand)]
    Instance(Box<IndexerInstanceCommand>),
    #[command(subcommand)]
    Read(Box<IndexerReadCommand>),
}

#[derive(Subcommand)]
pub(crate) enum IndexerImportCommand {
    Create(ImportJobCreateArgs),
    RunProwlarrApi(ImportJobRunProwlarrApiArgs),
    RunProwlarrBackup(ImportJobRunProwlarrBackupArgs),
    Status(ImportJobStatusArgs),
    Results(ImportJobResultsArgs),
}

#[derive(Subcommand)]
pub(crate) enum TorznabCommand {
    Create(TorznabCreateArgs),
    Rotate(TorznabRotateArgs),
    SetState(TorznabSetStateArgs),
    Delete(TorznabDeleteArgs),
}

#[derive(Subcommand)]
pub(crate) enum IndexerInstanceCommand {
    TestPrepare(IndexerInstanceTestPrepareArgs),
    TestFinalize(IndexerInstanceTestFinalizeArgs),
}

#[derive(Subcommand)]
pub(crate) enum IndexerReadCommand {
    Tags,
    Secrets,
    SearchProfiles,
    PolicySets,
    RoutingPolicies,
    RoutingPolicy(IndexerRoutingPolicyReadArgs),
    RateLimits,
    Instances,
    TorznabInstances,
    BackupExport,
    Connectivity(IndexerInstanceReadArgs),
    Reputation(IndexerInstanceReadArgs),
    HealthEvents(IndexerInstanceReadArgs),
    Rss(IndexerInstanceReadArgs),
    RssItems(IndexerInstanceRssItemsArgs),
}

#[derive(Subcommand)]
pub(crate) enum PolicyCommand {
    SetCreate(PolicySetCreateArgs),
    SetUpdate(PolicySetUpdateArgs),
    SetEnable(PolicySetEnableArgs),
    SetDisable(PolicySetDisableArgs),
    SetReorder(PolicySetReorderArgs),
    RuleCreate(Box<PolicyRuleCreateArgs>),
    RuleEnable(PolicyRuleEnableArgs),
    RuleDisable(PolicyRuleDisableArgs),
    RuleReorder(PolicyRuleReorderArgs),
}

#[derive(Args)]
pub(crate) struct PolicySetCreateArgs {
    #[arg(long, help = "Display name for the policy set")]
    pub display_name: String,
    #[arg(long, help = "Policy scope key (global, user, profile, request)")]
    pub scope: String,
    #[arg(long, default_value_t = true, help = "Enable policy set on creation")]
    pub enabled: bool,
}

#[derive(Args)]
pub(crate) struct PolicySetUpdateArgs {
    #[arg(value_parser = parse_policy_set_id, help = "Policy set public id")]
    pub policy_set_public_id: Uuid,
    #[arg(long, help = "Updated display name (optional)")]
    pub display_name: Option<String>,
}

#[derive(Args)]
pub(crate) struct PolicySetEnableArgs {
    #[arg(value_parser = parse_policy_set_id, help = "Policy set public id")]
    pub policy_set_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct PolicySetDisableArgs {
    #[arg(value_parser = parse_policy_set_id, help = "Policy set public id")]
    pub policy_set_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct PolicySetReorderArgs {
    #[arg(long, value_delimiter = ',', help = "Ordered policy set public ids")]
    pub ordered_policy_set_public_ids: Vec<Uuid>,
}

#[derive(Args)]
pub(crate) struct PolicyRuleCreateArgs {
    #[arg(value_parser = parse_policy_set_id, help = "Policy set public id")]
    pub policy_set_public_id: Uuid,
    #[arg(long, help = "Policy rule type key")]
    pub rule_type: String,
    #[arg(long, help = "Policy match field key")]
    pub match_field: String,
    #[arg(long, help = "Policy match operator key")]
    pub match_operator: String,
    #[arg(long, help = "Policy action key")]
    pub action: String,
    #[arg(long, help = "Policy severity key")]
    pub severity: String,
    #[arg(long, help = "Sort order for policy rule evaluation")]
    pub sort_order: i32,
    #[arg(long, help = "Match value text")]
    pub match_value_text: Option<String>,
    #[arg(long, help = "Match value integer")]
    pub match_value_int: Option<i32>,
    #[arg(long, help = "Match value UUID")]
    pub match_value_uuid: Option<Uuid>,
    #[arg(long, value_delimiter = ',', help = "Value-set items (text values)")]
    pub value_set_text: Vec<String>,
    #[arg(long, value_delimiter = ',', help = "Value-set items (int values)")]
    pub value_set_int: Vec<i32>,
    #[arg(long, value_delimiter = ',', help = "Value-set items (bigint values)")]
    pub value_set_bigint: Vec<i64>,
    #[arg(long, value_delimiter = ',', help = "Value-set items (UUID values)")]
    pub value_set_uuid: Vec<Uuid>,
    #[arg(long, help = "Enable case insensitive matching")]
    pub case_insensitive: bool,
    #[arg(long, help = "Policy rule rationale")]
    pub rationale: Option<String>,
    #[arg(long, help = "Expiry timestamp (RFC3339)")]
    pub expires_at: Option<String>,
}

#[derive(Args)]
pub(crate) struct PolicyRuleEnableArgs {
    #[arg(value_parser = parse_policy_rule_id, help = "Policy rule public id")]
    pub policy_rule_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct PolicyRuleDisableArgs {
    #[arg(value_parser = parse_policy_rule_id, help = "Policy rule public id")]
    pub policy_rule_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct PolicyRuleReorderArgs {
    #[arg(value_parser = parse_policy_set_id, help = "Policy set public id")]
    pub policy_set_public_id: Uuid,
    #[arg(long, value_delimiter = ',', help = "Ordered policy rule public ids")]
    pub ordered_policy_rule_public_ids: Vec<Uuid>,
}

#[derive(Args)]
pub(crate) struct ImportJobCreateArgs {
    #[arg(
        long,
        value_enum,
        help = "Import source (prowlarr_api or prowlarr_backup)"
    )]
    pub source: ImportSourceArg,
    #[arg(long, help = "Mark import job as dry-run only")]
    pub dry_run: bool,
    #[arg(long, help = "Target search profile public id (optional)")]
    pub target_search_profile: Option<Uuid>,
    #[arg(long, help = "Target Torznab instance public id (optional)")]
    pub target_torznab_instance: Option<Uuid>,
}

#[derive(Args)]
pub(crate) struct ImportJobRunProwlarrApiArgs {
    #[arg(value_parser = parse_import_job_id, help = "Import job public id")]
    pub import_job_public_id: Uuid,
    #[arg(long, help = "Prowlarr base URL")]
    pub prowlarr_url: String,
    #[arg(long, help = "Secret public id containing the Prowlarr API key")]
    pub prowlarr_api_key_secret_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct ImportJobRunProwlarrBackupArgs {
    #[arg(value_parser = parse_import_job_id, help = "Import job public id")]
    pub import_job_public_id: Uuid,
    #[arg(long, help = "Backup blob reference")]
    pub backup_blob_ref: String,
}

#[derive(Args)]
pub(crate) struct ImportJobStatusArgs {
    #[arg(value_parser = parse_import_job_id, help = "Import job public id")]
    pub import_job_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct ImportJobResultsArgs {
    #[arg(value_parser = parse_import_job_id, help = "Import job public id")]
    pub import_job_public_id: Uuid,
}

#[derive(Copy, Clone, Debug, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ImportSourceArg {
    ProwlarrApi,
    ProwlarrBackup,
}

impl ImportSourceArg {
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ProwlarrApi => "prowlarr_api",
            Self::ProwlarrBackup => "prowlarr_backup",
        }
    }
}

#[derive(Args)]
pub(crate) struct TorznabCreateArgs {
    #[arg(long, help = "Search profile public id to bind")]
    pub search_profile_public_id: Uuid,
    #[arg(long, help = "Display name for the instance")]
    pub display_name: String,
}

#[derive(Args)]
pub(crate) struct TorznabRotateArgs {
    #[arg(value_parser = parse_torznab_instance_id, help = "Torznab instance public id")]
    pub torznab_instance_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct TorznabSetStateArgs {
    #[arg(value_parser = parse_torznab_instance_id, help = "Torznab instance public id")]
    pub torznab_instance_public_id: Uuid,
    #[arg(long, help = "Enable or disable the instance")]
    pub enabled: bool,
}

#[derive(Args)]
pub(crate) struct TorznabDeleteArgs {
    #[arg(value_parser = parse_torznab_instance_id, help = "Torznab instance public id")]
    pub torznab_instance_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct IndexerInstanceTestPrepareArgs {
    #[arg(value_parser = parse_indexer_instance_id, help = "Indexer instance public id")]
    pub indexer_instance_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct IndexerInstanceTestFinalizeArgs {
    #[arg(value_parser = parse_indexer_instance_id, help = "Indexer instance public id")]
    pub indexer_instance_public_id: Uuid,
    #[arg(long, help = "Mark test as successful")]
    pub ok: bool,
    #[arg(long, help = "Error class label (optional)")]
    pub error_class: Option<String>,
    #[arg(long, help = "Error code label (optional)")]
    pub error_code: Option<String>,
    #[arg(long, help = "Detail string (optional)")]
    pub detail: Option<String>,
    #[arg(long, help = "Result count (optional)")]
    pub result_count: Option<i32>,
}

#[derive(Args)]
pub(crate) struct IndexerRoutingPolicyReadArgs {
    #[arg(value_parser = parse_routing_policy_id, help = "Routing policy public id")]
    pub routing_policy_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct IndexerInstanceReadArgs {
    #[arg(value_parser = parse_indexer_instance_id, help = "Indexer instance public id")]
    pub indexer_instance_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct IndexerInstanceRssItemsArgs {
    #[arg(value_parser = parse_indexer_instance_id, help = "Indexer instance public id")]
    pub indexer_instance_public_id: Uuid,
    #[arg(long, help = "Optional result limit")]
    pub limit: Option<i32>,
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
    use crate::client::{CliError, HEADER_API_KEY, parse_api_key, parse_url, timestamp_now_ms};
    use anyhow::{Result, anyhow};
    use httpmock::MockServer;
    use httpmock::prelude::*;
    use revaer_config::validate::default_local_networks;
    use std::{fs, path::PathBuf};

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
        assert_eq!(
            command_label(&Command::Indexer(IndexerCommand::Import(
                IndexerImportCommand::Create(ImportJobCreateArgs {
                    source: ImportSourceArg::ProwlarrApi,
                    dry_run: false,
                    target_search_profile: None,
                    target_torznab_instance: None,
                })
            ))),
            "indexer_import_create"
        );
        assert_eq!(
            command_label(&Command::Indexer(IndexerCommand::Torznab(
                TorznabCommand::Rotate(TorznabRotateArgs {
                    torznab_instance_public_id: Uuid::nil(),
                })
            ))),
            "indexer_torznab_rotate"
        );
        assert_eq!(
            command_label(&Command::Indexer(IndexerCommand::Policy(Box::new(
                PolicyCommand::SetCreate(PolicySetCreateArgs {
                    display_name: "Demo".to_string(),
                    scope: "global".to_string(),
                    enabled: true,
                })
            )))),
            "indexer_policy_set_create"
        );
        assert_eq!(
            command_label(&Command::Indexer(IndexerCommand::Instance(Box::new(
                IndexerInstanceCommand::TestPrepare(IndexerInstanceTestPrepareArgs {
                    indexer_instance_public_id: Uuid::nil(),
                })
            )))),
            "indexer_instance_test_prepare"
        );
        assert_eq!(
            command_label(&Command::Indexer(IndexerCommand::Read(Box::new(
                IndexerReadCommand::Tags
            )))),
            "indexer_read_tags"
        );
        assert_eq!(
            command_label(&Command::Indexer(IndexerCommand::Read(Box::new(
                IndexerReadCommand::RoutingPolicy(IndexerRoutingPolicyReadArgs {
                    routing_policy_public_id: Uuid::nil(),
                })
            )))),
            "indexer_read_routing_policy"
        );
    }

    #[test]
    fn timestamp_now_ms_returns_positive_value() {
        assert!(timestamp_now_ms() > 0);
    }

    #[test]
    fn parse_existing_file_verifies_path() -> Result<()> {
        let tmp = server_root()?.join(format!("revaer-cli-{}.txt", Uuid::new_v4()));
        std::fs::write(&tmp, b"ok")?;
        let tmp_path = tmp.to_str().ok_or_else(|| anyhow!("invalid temp path"))?;
        assert!(parse_existing_file(tmp_path).is_ok());
        let missing = server_root()?.join(format!("revaer-cli-missing-{}.txt", Uuid::new_v4()));
        let missing_path = missing
            .to_str()
            .ok_or_else(|| anyhow!("invalid missing path"))?;
        assert!(parse_existing_file(missing_path).is_err());
        std::fs::remove_file(&tmp)?;
        Ok(())
    }

    #[test]
    fn parse_existing_directory_verifies_path() -> Result<()> {
        let dir = server_root()?;
        let dir_path = dir.to_str().ok_or_else(|| anyhow!("invalid dir path"))?;
        assert!(parse_existing_directory(dir_path).is_ok());
        let missing = dir.join(format!("revaer-cli-dir-{}", Uuid::new_v4()));
        let missing_path = missing
            .to_str()
            .ok_or_else(|| anyhow!("invalid missing dir path"))?;
        assert!(parse_existing_directory(missing_path).is_err());
        Ok(())
    }

    fn sample_snapshot() -> Result<revaer_config::ConfigSnapshot> {
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
        Ok(revaer_config::ConfigSnapshot {
            revision: 1,
            app_profile: revaer_config::AppProfile {
                id: Uuid::new_v4(),
                instance_name: "demo".into(),
                mode: revaer_config::AppMode::Active,
                auth_mode: revaer_config::AppAuthMode::ApiKey,
                version: 1,
                http_port: 7070,
                bind_addr: "127.0.0.1".parse().map_err(|_| anyhow!("bind addr"))?,
                local_networks: default_local_networks(),
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
    async fn run_with_cli_executes_config_get() -> Result<()> {
        let server = MockServer::start_async().await;
        let snapshot = sample_snapshot()?;
        let payload = serde_json::to_value(&snapshot)?;
        let config_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/v1/config")
                .header(HEADER_API_KEY, "key:secret");
            then.status(200).json_body(payload);
        });

        let cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "config",
            "get",
        ]);

        let exit_code = run_with_cli(cli).await;
        config_mock.assert();
        assert_eq!(exit_code, 0);
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_reports_validation_errors() -> Result<()> {
        let server = MockServer::start_async().await;
        let cli = Cli::parse_from(["revaer", "--api-url", &server.base_url(), "config", "get"]);
        let exit_code = run_with_cli(cli).await;
        assert_eq!(exit_code, 2);
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_indexer_read_tags() -> Result<()> {
        let server = MockServer::start_async().await;
        let payload = serde_json::json!({
            "tags": [{
                "tag_public_id": Uuid::new_v4(),
                "tag_key": "anime",
                "display_name": "Anime",
                "updated_at": "2026-04-03T00:00:00Z"
            }]
        });
        let tags_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/v1/indexers/tags")
                .header(HEADER_API_KEY, "key:secret");
            then.status(200).json_body(payload);
        });

        let cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "indexer",
            "read",
            "tags",
        ]);

        let exit_code = run_with_cli(cli).await;
        tags_mock.assert();
        assert_eq!(exit_code, 0);
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_indexer_read_rss_items_with_limit() -> Result<()> {
        let server = MockServer::start_async().await;
        let instance_id = Uuid::new_v4();
        let payload = serde_json::json!({
            "items": [{
                "item_guid": "guid-1",
                "infohash_v1": null,
                "infohash_v2": null,
                "magnet_hash": null,
                "first_seen_at": "2026-04-03T00:00:00Z"
            }]
        });
        let rss_mock = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/v1/indexers/instances/{instance_id}/rss/items"))
                .query_param("limit", "5")
                .header(HEADER_API_KEY, "key:secret");
            then.status(200).json_body(payload);
        });

        let cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "indexer",
            "read",
            "rss-items",
            &instance_id.to_string(),
            "--limit",
            "5",
        ]);

        let exit_code = run_with_cli(cli).await;
        rss_mock.assert();
        assert_eq!(exit_code, 0);
        Ok(())
    }
}
