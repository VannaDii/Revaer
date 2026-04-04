//! Command-line client for interacting with a Revaer server instance.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use reqwest::Url;
use serde::Deserialize;
use uuid::Uuid;

use crate::client::{AppContext, CliDependencies, CliResult, parse_api_key, parse_url};
use crate::commands::indexers::{
    parse_health_notification_hook_id, parse_import_job_id, parse_indexer_instance_id,
    parse_policy_rule_id, parse_policy_set_id, parse_rate_limit_policy_id, parse_routing_policy_id,
    parse_search_profile_id, parse_torznab_instance_id,
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
        IndexerCommand::Tag(tag_command) => dispatch_indexer_tag(*tag_command, deps, output).await,
        IndexerCommand::Secret(secret_command) => {
            dispatch_indexer_secret(*secret_command, deps, output).await
        }
        IndexerCommand::HealthNotification(health_notification_command) => {
            dispatch_indexer_health_notification(*health_notification_command, deps, output).await
        }
        IndexerCommand::RoutingPolicy(routing_command) => {
            dispatch_indexer_routing_policy(*routing_command, deps, output).await
        }
        IndexerCommand::RateLimit(rate_limit_command) => {
            dispatch_indexer_rate_limit(*rate_limit_command, deps, output).await
        }
        IndexerCommand::SearchProfile(search_profile_command) => {
            dispatch_indexer_search_profile(*search_profile_command, deps, output).await
        }
        IndexerCommand::Backup(backup_command) => {
            dispatch_indexer_backup(*backup_command, deps, output).await
        }
        IndexerCommand::Rss(rss_command) => dispatch_indexer_rss(*rss_command, deps, output).await,
        IndexerCommand::CategoryMapping(mapping_command) => {
            dispatch_indexer_category_mapping(*mapping_command, deps).await
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

async fn dispatch_indexer_tag(
    command: TagCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        TagCommand::Create(args) => indexers::handle_tag_create(deps, args, output).await,
        TagCommand::Update(args) => indexers::handle_tag_update(deps, args, output).await,
        TagCommand::Delete(args) => indexers::handle_tag_delete(deps, args).await,
    }
}

async fn dispatch_indexer_secret(
    command: SecretCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        SecretCommand::Create(args) => indexers::handle_secret_create(deps, args, output).await,
        SecretCommand::Rotate(args) => indexers::handle_secret_rotate(deps, args, output).await,
        SecretCommand::Revoke(args) => indexers::handle_secret_revoke(deps, args).await,
    }
}

async fn dispatch_indexer_health_notification(
    command: HealthNotificationCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        HealthNotificationCommand::Create(args) => {
            indexers::handle_health_notification_hook_create(deps, args, output).await
        }
        HealthNotificationCommand::Update(args) => {
            indexers::handle_health_notification_hook_update(deps, args, output).await
        }
        HealthNotificationCommand::Delete(args) => {
            indexers::handle_health_notification_hook_delete(deps, args).await
        }
    }
}

async fn dispatch_indexer_category_mapping(
    command: CategoryMappingCommand,
    deps: &AppContext,
) -> CliResult<()> {
    match command {
        CategoryMappingCommand::TrackerUpsert(args) => {
            indexers::handle_tracker_category_mapping_upsert(deps, args).await
        }
        CategoryMappingCommand::TrackerDelete(args) => {
            indexers::handle_tracker_category_mapping_delete(deps, args).await
        }
        CategoryMappingCommand::MediaDomainUpsert(args) => {
            indexers::handle_media_domain_mapping_upsert(deps, args).await
        }
        CategoryMappingCommand::MediaDomainDelete(args) => {
            indexers::handle_media_domain_mapping_delete(deps, args).await
        }
    }
}

async fn dispatch_indexer_routing_policy(
    command: RoutingPolicyCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        RoutingPolicyCommand::Create(args) => {
            indexers::handle_routing_policy_create(deps, args, output).await
        }
        RoutingPolicyCommand::SetParam(args) => {
            indexers::handle_routing_policy_set_param(deps, args).await
        }
        RoutingPolicyCommand::BindSecret(args) => {
            indexers::handle_routing_policy_bind_secret(deps, args).await
        }
    }
}

async fn dispatch_indexer_rate_limit(
    command: RateLimitCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        RateLimitCommand::Create(args) => {
            indexers::handle_rate_limit_policy_create(deps, args, output).await
        }
        RateLimitCommand::Update(args) => {
            indexers::handle_rate_limit_policy_update(deps, args, output).await
        }
        RateLimitCommand::Delete(args) => {
            indexers::handle_rate_limit_policy_delete(deps, args).await
        }
        RateLimitCommand::AssignInstance(args) => {
            indexers::handle_rate_limit_assign_instance(deps, args).await
        }
        RateLimitCommand::AssignRouting(args) => {
            indexers::handle_rate_limit_assign_routing(deps, args).await
        }
    }
}

async fn dispatch_indexer_search_profile(
    command: SearchProfileCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        SearchProfileCommand::Create(args) => {
            indexers::handle_search_profile_create(deps, args, output).await
        }
        SearchProfileCommand::Update(args) => {
            indexers::handle_search_profile_update(deps, args, output).await
        }
        SearchProfileCommand::SetDefault(args) => {
            indexers::handle_search_profile_set_default(deps, args, output).await
        }
        SearchProfileCommand::SetDefaultDomain(args) => {
            indexers::handle_search_profile_set_default_domain(deps, args, output).await
        }
        SearchProfileCommand::SetMediaDomains(args) => {
            indexers::handle_search_profile_set_media_domains(deps, args).await
        }
        SearchProfileCommand::AddPolicySet(args) => {
            indexers::handle_search_profile_add_policy_set(deps, args).await
        }
        SearchProfileCommand::RemovePolicySet(args) => {
            indexers::handle_search_profile_remove_policy_set(deps, args).await
        }
        SearchProfileCommand::SetIndexerAllow(args) => {
            indexers::handle_search_profile_set_indexer_allow(deps, args).await
        }
        SearchProfileCommand::SetIndexerBlock(args) => {
            indexers::handle_search_profile_set_indexer_block(deps, args).await
        }
        SearchProfileCommand::SetTagAllow(args) => {
            indexers::handle_search_profile_set_tag_allow(deps, args).await
        }
        SearchProfileCommand::SetTagBlock(args) => {
            indexers::handle_search_profile_set_tag_block(deps, args).await
        }
        SearchProfileCommand::SetTagPrefer(args) => {
            indexers::handle_search_profile_set_tag_prefer(deps, args).await
        }
    }
}

async fn dispatch_indexer_backup(
    command: BackupCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        BackupCommand::Restore(args) => {
            indexers::handle_indexer_backup_restore(deps, args, output).await
        }
    }
}

async fn dispatch_indexer_rss(
    command: RssCommand,
    deps: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    match command {
        RssCommand::Set(args) => indexers::handle_indexer_rss_set(deps, args, output).await,
        RssCommand::MarkSeen(args) => {
            indexers::handle_indexer_rss_mark_seen(deps, args, output).await
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
        IndexerReadCommand::HealthNotifications => {
            indexers::handle_health_notification_hook_list(deps, output).await
        }
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
        Command::Indexer(indexer_command) => command_label_indexer(indexer_command),
    }
}

fn command_label_indexer(command: &IndexerCommand) -> &'static str {
    match command {
        IndexerCommand::Import(import_command) => match import_command {
            IndexerImportCommand::Create(_) => "indexer_import_create",
            IndexerImportCommand::RunProwlarrApi(_) => "indexer_import_run_prowlarr_api",
            IndexerImportCommand::RunProwlarrBackup(_) => "indexer_import_run_prowlarr_backup",
            IndexerImportCommand::Status(_) => "indexer_import_status",
            IndexerImportCommand::Results(_) => "indexer_import_results",
        },
        IndexerCommand::Tag(tag_command) => command_label_tag(tag_command),
        IndexerCommand::Secret(secret_command) => command_label_secret(secret_command),
        IndexerCommand::HealthNotification(health_notification_command) => {
            command_label_health_notification(health_notification_command)
        }
        IndexerCommand::RoutingPolicy(routing_command) => {
            command_label_routing_policy(routing_command)
        }
        IndexerCommand::RateLimit(rate_limit_command) => {
            command_label_rate_limit(rate_limit_command)
        }
        IndexerCommand::SearchProfile(search_profile_command) => {
            command_label_search_profile(search_profile_command)
        }
        IndexerCommand::Backup(backup_command) => command_label_backup(backup_command),
        IndexerCommand::Rss(rss_command) => command_label_rss(rss_command),
        IndexerCommand::CategoryMapping(mapping_command) => {
            command_label_category_mapping(mapping_command)
        }
        IndexerCommand::Torznab(torznab_command) => match torznab_command {
            TorznabCommand::Create(_) => "indexer_torznab_create",
            TorznabCommand::Rotate(_) => "indexer_torznab_rotate",
            TorznabCommand::SetState(_) => "indexer_torznab_set_state",
            TorznabCommand::Delete(_) => "indexer_torznab_delete",
        },
        IndexerCommand::Policy(policy_command) => command_label_policy(policy_command),
        IndexerCommand::Instance(instance_command) => match instance_command.as_ref() {
            IndexerInstanceCommand::TestPrepare(_) => "indexer_instance_test_prepare",
            IndexerInstanceCommand::TestFinalize(_) => "indexer_instance_test_finalize",
        },
        IndexerCommand::Read(read_command) => command_label_indexer_read(read_command),
    }
}

const fn command_label_routing_policy(command: &RoutingPolicyCommand) -> &'static str {
    match command {
        RoutingPolicyCommand::Create(_) => "indexer_routing_policy_create",
        RoutingPolicyCommand::SetParam(_) => "indexer_routing_policy_set_param",
        RoutingPolicyCommand::BindSecret(_) => "indexer_routing_policy_bind_secret",
    }
}

const fn command_label_rate_limit(command: &RateLimitCommand) -> &'static str {
    match command {
        RateLimitCommand::Create(_) => "indexer_rate_limit_create",
        RateLimitCommand::Update(_) => "indexer_rate_limit_update",
        RateLimitCommand::Delete(_) => "indexer_rate_limit_delete",
        RateLimitCommand::AssignInstance(_) => "indexer_rate_limit_assign_instance",
        RateLimitCommand::AssignRouting(_) => "indexer_rate_limit_assign_routing",
    }
}

const fn command_label_search_profile(command: &SearchProfileCommand) -> &'static str {
    match command {
        SearchProfileCommand::Create(_) => "indexer_search_profile_create",
        SearchProfileCommand::Update(_) => "indexer_search_profile_update",
        SearchProfileCommand::SetDefault(_) => "indexer_search_profile_set_default",
        SearchProfileCommand::SetDefaultDomain(_) => "indexer_search_profile_set_default_domain",
        SearchProfileCommand::SetMediaDomains(_) => "indexer_search_profile_set_media_domains",
        SearchProfileCommand::AddPolicySet(_) => "indexer_search_profile_add_policy_set",
        SearchProfileCommand::RemovePolicySet(_) => "indexer_search_profile_remove_policy_set",
        SearchProfileCommand::SetIndexerAllow(_) => "indexer_search_profile_set_indexer_allow",
        SearchProfileCommand::SetIndexerBlock(_) => "indexer_search_profile_set_indexer_block",
        SearchProfileCommand::SetTagAllow(_) => "indexer_search_profile_set_tag_allow",
        SearchProfileCommand::SetTagBlock(_) => "indexer_search_profile_set_tag_block",
        SearchProfileCommand::SetTagPrefer(_) => "indexer_search_profile_set_tag_prefer",
    }
}

const fn command_label_backup(command: &BackupCommand) -> &'static str {
    match command {
        BackupCommand::Restore(_) => "indexer_backup_restore",
    }
}

const fn command_label_rss(command: &RssCommand) -> &'static str {
    match command {
        RssCommand::Set(_) => "indexer_rss_set",
        RssCommand::MarkSeen(_) => "indexer_rss_mark_seen",
    }
}

const fn command_label_tag(command: &TagCommand) -> &'static str {
    match command {
        TagCommand::Create(_) => "indexer_tag_create",
        TagCommand::Update(_) => "indexer_tag_update",
        TagCommand::Delete(_) => "indexer_tag_delete",
    }
}

const fn command_label_secret(command: &SecretCommand) -> &'static str {
    match command {
        SecretCommand::Create(_) => "indexer_secret_create",
        SecretCommand::Rotate(_) => "indexer_secret_rotate",
        SecretCommand::Revoke(_) => "indexer_secret_revoke",
    }
}

const fn command_label_health_notification(command: &HealthNotificationCommand) -> &'static str {
    match command {
        HealthNotificationCommand::Create(_) => "indexer_health_notification_create",
        HealthNotificationCommand::Update(_) => "indexer_health_notification_update",
        HealthNotificationCommand::Delete(_) => "indexer_health_notification_delete",
    }
}

const fn command_label_category_mapping(command: &CategoryMappingCommand) -> &'static str {
    match command {
        CategoryMappingCommand::TrackerUpsert(_) => "indexer_category_mapping_tracker_upsert",
        CategoryMappingCommand::TrackerDelete(_) => "indexer_category_mapping_tracker_delete",
        CategoryMappingCommand::MediaDomainUpsert(_) => {
            "indexer_category_mapping_media_domain_upsert"
        }
        CategoryMappingCommand::MediaDomainDelete(_) => {
            "indexer_category_mapping_media_domain_delete"
        }
    }
}

const fn command_label_policy(command: &PolicyCommand) -> &'static str {
    match command {
        PolicyCommand::SetCreate(_) => "indexer_policy_set_create",
        PolicyCommand::SetUpdate(_) => "indexer_policy_set_update",
        PolicyCommand::SetEnable(_) => "indexer_policy_set_enable",
        PolicyCommand::SetDisable(_) => "indexer_policy_set_disable",
        PolicyCommand::SetReorder(_) => "indexer_policy_set_reorder",
        PolicyCommand::RuleCreate(_) => "indexer_policy_rule_create",
        PolicyCommand::RuleEnable(_) => "indexer_policy_rule_enable",
        PolicyCommand::RuleDisable(_) => "indexer_policy_rule_disable",
        PolicyCommand::RuleReorder(_) => "indexer_policy_rule_reorder",
    }
}

const fn command_label_indexer_read(command: &IndexerReadCommand) -> &'static str {
    match command {
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
        IndexerReadCommand::HealthNotifications => "indexer_read_health_notifications",
        IndexerReadCommand::Rss(_) => "indexer_read_rss",
        IndexerReadCommand::RssItems(_) => "indexer_read_rss_items",
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
    Tag(Box<TagCommand>),
    #[command(subcommand)]
    Secret(Box<SecretCommand>),
    #[command(subcommand)]
    HealthNotification(Box<HealthNotificationCommand>),
    #[command(subcommand)]
    RoutingPolicy(Box<RoutingPolicyCommand>),
    #[command(subcommand)]
    RateLimit(Box<RateLimitCommand>),
    #[command(subcommand)]
    SearchProfile(Box<SearchProfileCommand>),
    #[command(subcommand)]
    Backup(Box<BackupCommand>),
    #[command(subcommand)]
    Rss(Box<RssCommand>),
    #[command(subcommand)]
    CategoryMapping(Box<CategoryMappingCommand>),
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
pub(crate) enum TagCommand {
    Create(TagCreateArgs),
    Update(TagUpdateArgs),
    Delete(TagDeleteArgs),
}

#[derive(Subcommand)]
pub(crate) enum SecretCommand {
    Create(SecretCreateArgs),
    Rotate(SecretRotateArgs),
    Revoke(SecretRevokeArgs),
}

#[derive(Subcommand)]
pub(crate) enum HealthNotificationCommand {
    Create(HealthNotificationCreateArgs),
    Update(HealthNotificationUpdateArgs),
    Delete(HealthNotificationDeleteArgs),
}

#[derive(Subcommand)]
pub(crate) enum CategoryMappingCommand {
    TrackerUpsert(TrackerCategoryMappingUpsertArgs),
    TrackerDelete(TrackerCategoryMappingDeleteArgs),
    MediaDomainUpsert(MediaDomainMappingUpsertArgs),
    MediaDomainDelete(MediaDomainMappingDeleteArgs),
}

#[derive(Subcommand)]
pub(crate) enum RoutingPolicyCommand {
    Create(RoutingPolicyCreateArgs),
    SetParam(RoutingPolicySetParamArgs),
    BindSecret(RoutingPolicyBindSecretArgs),
}

#[derive(Subcommand)]
pub(crate) enum RateLimitCommand {
    Create(RateLimitCreateArgs),
    Update(RateLimitUpdateArgs),
    Delete(RateLimitDeleteArgs),
    AssignInstance(RateLimitAssignInstanceArgs),
    AssignRouting(RateLimitAssignRoutingArgs),
}

#[derive(Subcommand)]
pub(crate) enum SearchProfileCommand {
    Create(SearchProfileCreateArgs),
    Update(SearchProfileUpdateArgs),
    SetDefault(SearchProfileSetDefaultArgs),
    SetDefaultDomain(SearchProfileSetDefaultDomainArgs),
    SetMediaDomains(SearchProfileSetMediaDomainsArgs),
    AddPolicySet(SearchProfilePolicySetArgs),
    RemovePolicySet(SearchProfilePolicySetArgs),
    SetIndexerAllow(SearchProfileIndexerSetArgs),
    SetIndexerBlock(SearchProfileIndexerSetArgs),
    SetTagAllow(SearchProfileTagSetArgs),
    SetTagBlock(SearchProfileTagSetArgs),
    SetTagPrefer(SearchProfileTagSetArgs),
}

#[derive(Subcommand)]
pub(crate) enum BackupCommand {
    Restore(BackupRestoreArgs),
}

#[derive(Subcommand)]
pub(crate) enum RssCommand {
    Set(IndexerRssSetArgs),
    MarkSeen(IndexerRssMarkSeenArgs),
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
    HealthNotifications,
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
pub(crate) struct TagCreateArgs {
    #[arg(long, help = "Unique lowercase tag key")]
    pub tag_key: String,
    #[arg(long, help = "Operator-facing display name")]
    pub display_name: String,
}

#[derive(Args)]
pub(crate) struct TagUpdateArgs {
    #[arg(long, help = "Tag public id (optional when tag-key is provided)")]
    pub tag_public_id: Option<Uuid>,
    #[arg(long, help = "Tag key (optional when tag-public-id is provided)")]
    pub tag_key: Option<String>,
    #[arg(long, help = "Updated display name")]
    pub display_name: String,
}

#[derive(Args)]
pub(crate) struct TagDeleteArgs {
    #[arg(long, help = "Tag public id (optional when tag-key is provided)")]
    pub tag_public_id: Option<Uuid>,
    #[arg(long, help = "Tag key (optional when tag-public-id is provided)")]
    pub tag_key: Option<String>,
}

#[derive(Args)]
pub(crate) struct SecretCreateArgs {
    #[arg(long, help = "Secret type label")]
    pub secret_type: String,
    #[arg(long, help = "Plaintext secret value")]
    pub secret_value: String,
}

#[derive(Args)]
pub(crate) struct SecretRotateArgs {
    #[arg(long, help = "Secret public id")]
    pub secret_public_id: Uuid,
    #[arg(long, help = "New plaintext secret value")]
    pub secret_value: String,
}

#[derive(Args)]
pub(crate) struct SecretRevokeArgs {
    #[arg(long, help = "Secret public id")]
    pub secret_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct HealthNotificationCreateArgs {
    #[arg(long, help = "Notification channel (email or webhook)")]
    pub channel: String,
    #[arg(long, help = "Operator-facing display name")]
    pub display_name: String,
    #[arg(
        long,
        help = "Lowest triggering status (degraded, failing, quarantined)"
    )]
    pub status_threshold: String,
    #[arg(long, help = "Webhook URL for webhook hooks")]
    pub webhook_url: Option<String>,
    #[arg(long, help = "Email address for email hooks")]
    pub email: Option<String>,
}

#[derive(Args)]
pub(crate) struct HealthNotificationUpdateArgs {
    #[arg(
        value_parser = parse_health_notification_hook_id,
        help = "Health notification hook public id"
    )]
    pub indexer_health_notification_hook_public_id: Uuid,
    #[arg(long, help = "Updated display name")]
    pub display_name: Option<String>,
    #[arg(long, help = "Updated lowest triggering status")]
    pub status_threshold: Option<String>,
    #[arg(long, help = "Updated webhook URL for webhook hooks")]
    pub webhook_url: Option<String>,
    #[arg(long, help = "Updated email address for email hooks")]
    pub email: Option<String>,
    #[arg(long, help = "Override enabled state")]
    pub is_enabled: Option<bool>,
}

#[derive(Args)]
pub(crate) struct HealthNotificationDeleteArgs {
    #[arg(
        value_parser = parse_health_notification_hook_id,
        help = "Health notification hook public id"
    )]
    pub indexer_health_notification_hook_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct TrackerCategoryMappingUpsertArgs {
    #[arg(
        long,
        help = "Optional Torznab instance public id for app-scoped overrides"
    )]
    pub torznab_instance_public_id: Option<Uuid>,
    #[arg(long, help = "Optional definition upstream slug")]
    pub indexer_definition_upstream_slug: Option<String>,
    #[arg(long, help = "Optional indexer instance public id")]
    pub indexer_instance_public_id: Option<Uuid>,
    #[arg(long, help = "Tracker category id")]
    pub tracker_category: i32,
    #[arg(long, help = "Optional tracker subcategory id")]
    pub tracker_subcategory: Option<i32>,
    #[arg(long, help = "Torznab category id")]
    pub torznab_cat_id: i32,
    #[arg(long, help = "Optional media domain key")]
    pub media_domain_key: Option<String>,
}

#[derive(Args)]
pub(crate) struct TrackerCategoryMappingDeleteArgs {
    #[arg(
        long,
        help = "Optional Torznab instance public id for app-scoped overrides"
    )]
    pub torznab_instance_public_id: Option<Uuid>,
    #[arg(long, help = "Optional definition upstream slug")]
    pub indexer_definition_upstream_slug: Option<String>,
    #[arg(long, help = "Optional indexer instance public id")]
    pub indexer_instance_public_id: Option<Uuid>,
    #[arg(long, help = "Tracker category id")]
    pub tracker_category: i32,
    #[arg(long, help = "Optional tracker subcategory id")]
    pub tracker_subcategory: Option<i32>,
}

#[derive(Args)]
pub(crate) struct MediaDomainMappingUpsertArgs {
    #[arg(long, help = "Media domain key")]
    pub media_domain_key: String,
    #[arg(long, help = "Torznab category id")]
    pub torznab_cat_id: i32,
    #[arg(long, help = "Optional primary flag value (true or false)")]
    pub is_primary: Option<bool>,
}

#[derive(Args)]
pub(crate) struct MediaDomainMappingDeleteArgs {
    #[arg(long, help = "Media domain key")]
    pub media_domain_key: String,
    #[arg(long, help = "Torznab category id")]
    pub torznab_cat_id: i32,
}

#[derive(Args)]
pub(crate) struct RoutingPolicyCreateArgs {
    #[arg(long, help = "Display name for the routing policy")]
    pub display_name: String,
    #[arg(long, help = "Routing mode key")]
    pub mode: String,
}

#[derive(Args)]
pub(crate) struct RoutingPolicySetParamArgs {
    #[arg(value_parser = parse_routing_policy_id, help = "Routing policy public id")]
    pub routing_policy_public_id: Uuid,
    #[arg(long, help = "Parameter key")]
    pub param_key: String,
    #[arg(long, help = "Optional plain-text parameter value")]
    pub value_plain: Option<String>,
    #[arg(long, help = "Optional integer parameter value")]
    pub value_int: Option<i32>,
    #[arg(long, help = "Optional boolean parameter value")]
    pub value_bool: Option<bool>,
}

#[derive(Args)]
pub(crate) struct RoutingPolicyBindSecretArgs {
    #[arg(value_parser = parse_routing_policy_id, help = "Routing policy public id")]
    pub routing_policy_public_id: Uuid,
    #[arg(long, help = "Parameter key")]
    pub param_key: String,
    #[arg(long, help = "Secret public id")]
    pub secret_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct RateLimitCreateArgs {
    #[arg(long, help = "Display name for the rate-limit policy")]
    pub display_name: String,
    #[arg(long, help = "Requests per minute")]
    pub rpm: i32,
    #[arg(long, help = "Burst token count")]
    pub burst: i32,
    #[arg(long, help = "Concurrent request limit")]
    pub concurrent: i32,
}

#[derive(Args)]
pub(crate) struct RateLimitUpdateArgs {
    #[arg(value_parser = parse_rate_limit_policy_id, help = "Rate-limit policy public id")]
    pub rate_limit_policy_public_id: Uuid,
    #[arg(long, help = "Updated display name")]
    pub display_name: Option<String>,
    #[arg(long, help = "Updated requests per minute")]
    pub rpm: Option<i32>,
    #[arg(long, help = "Updated burst token count")]
    pub burst: Option<i32>,
    #[arg(long, help = "Updated concurrent request limit")]
    pub concurrent: Option<i32>,
}

#[derive(Args)]
pub(crate) struct RateLimitDeleteArgs {
    #[arg(value_parser = parse_rate_limit_policy_id, help = "Rate-limit policy public id")]
    pub rate_limit_policy_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct RateLimitAssignInstanceArgs {
    #[arg(value_parser = parse_indexer_instance_id, help = "Indexer instance public id")]
    pub indexer_instance_public_id: Uuid,
    #[arg(long, help = "Rate-limit policy public id (omit to clear)")]
    pub rate_limit_policy_public_id: Option<Uuid>,
}

#[derive(Args)]
pub(crate) struct RateLimitAssignRoutingArgs {
    #[arg(value_parser = parse_routing_policy_id, help = "Routing policy public id")]
    pub routing_policy_public_id: Uuid,
    #[arg(long, help = "Rate-limit policy public id (omit to clear)")]
    pub rate_limit_policy_public_id: Option<Uuid>,
}

#[derive(Args)]
pub(crate) struct SearchProfileCreateArgs {
    #[arg(long, help = "Display name for the search profile")]
    pub display_name: String,
    #[arg(long, help = "Mark this profile as default")]
    pub is_default: bool,
    #[arg(long, help = "Optional page size override")]
    pub page_size: Option<i32>,
    #[arg(long, help = "Optional default media-domain key")]
    pub default_media_domain_key: Option<String>,
    #[arg(long, help = "Optional user public id for scoped profiles")]
    pub user_public_id: Option<Uuid>,
}

#[derive(Args)]
pub(crate) struct SearchProfileUpdateArgs {
    #[arg(value_parser = parse_search_profile_id, help = "Search profile public id")]
    pub search_profile_public_id: Uuid,
    #[arg(long, help = "Updated display name")]
    pub display_name: Option<String>,
    #[arg(long, help = "Updated page size")]
    pub page_size: Option<i32>,
}

#[derive(Args)]
pub(crate) struct SearchProfileSetDefaultArgs {
    #[arg(value_parser = parse_search_profile_id, help = "Search profile public id")]
    pub search_profile_public_id: Uuid,
    #[arg(long, help = "Optional page size override")]
    pub page_size: Option<i32>,
}

#[derive(Args)]
pub(crate) struct SearchProfileSetDefaultDomainArgs {
    #[arg(value_parser = parse_search_profile_id, help = "Search profile public id")]
    pub search_profile_public_id: Uuid,
    #[arg(long, help = "Default media-domain key (omit to clear)")]
    pub default_media_domain_key: Option<String>,
}

#[derive(Args)]
pub(crate) struct SearchProfileSetMediaDomainsArgs {
    #[arg(value_parser = parse_search_profile_id, help = "Search profile public id")]
    pub search_profile_public_id: Uuid,
    #[arg(long, value_delimiter = ',', help = "Allowed media-domain keys")]
    pub media_domain_keys: Vec<String>,
}

#[derive(Args)]
pub(crate) struct SearchProfilePolicySetArgs {
    #[arg(value_parser = parse_search_profile_id, help = "Search profile public id")]
    pub search_profile_public_id: Uuid,
    #[arg(long, value_parser = parse_policy_set_id, help = "Policy set public id")]
    pub policy_set_public_id: Uuid,
}

#[derive(Args)]
pub(crate) struct SearchProfileIndexerSetArgs {
    #[arg(value_parser = parse_search_profile_id, help = "Search profile public id")]
    pub search_profile_public_id: Uuid,
    #[arg(long, value_delimiter = ',', help = "Indexer instance public ids")]
    pub indexer_instance_public_ids: Vec<Uuid>,
}

#[derive(Args)]
pub(crate) struct SearchProfileTagSetArgs {
    #[arg(value_parser = parse_search_profile_id, help = "Search profile public id")]
    pub search_profile_public_id: Uuid,
    #[arg(long, value_delimiter = ',', help = "Tag public ids")]
    pub tag_public_ids: Vec<Uuid>,
    #[arg(long, value_delimiter = ',', help = "Tag keys")]
    pub tag_keys: Vec<String>,
}

#[derive(Args)]
pub(crate) struct BackupRestoreArgs {
    #[arg(long, value_parser = parse_existing_file, help = "Path to backup snapshot JSON")]
    pub file: PathBuf,
}

#[derive(Args)]
pub(crate) struct IndexerRssSetArgs {
    #[arg(value_parser = parse_indexer_instance_id, help = "Indexer instance public id")]
    pub indexer_instance_public_id: Uuid,
    #[arg(long, help = "Enable or disable the RSS subscription")]
    pub is_enabled: bool,
    #[arg(long, help = "Optional poll interval override in seconds")]
    pub interval_seconds: Option<i32>,
}

#[derive(Args)]
pub(crate) struct IndexerRssMarkSeenArgs {
    #[arg(value_parser = parse_indexer_instance_id, help = "Indexer instance public id")]
    pub indexer_instance_public_id: Uuid,
    #[arg(long, help = "Optional feed GUID")]
    pub item_guid: Option<String>,
    #[arg(long, help = "Optional v1 infohash")]
    pub infohash_v1: Option<String>,
    #[arg(long, help = "Optional v2 infohash")]
    pub infohash_v2: Option<String>,
    #[arg(long, help = "Optional magnet hash")]
    pub magnet_hash: Option<String>,
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
    use crate::client::{
        CliError, HEADER_API_KEY, HEADER_SETUP_TOKEN, parse_api_key, parse_url, timestamp_now_ms,
    };
    use anyhow::{Result, anyhow};
    use chrono::Utc;
    use httpmock::MockServer;
    use httpmock::prelude::*;
    use revaer_api::models::{
        TorrentDetail, TorrentFileView, TorrentProgressView, TorrentRatesView, TorrentStateKind,
        TorrentStateView, TorrentSummary,
    };
    use revaer_config::validate::default_local_networks;
    use revaer_events::{Event, EventEnvelope};
    use std::{fs, path::PathBuf};
    use tokio::time::{Duration, timeout};

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

    fn temp_path(prefix: &str, extension: &str) -> Result<PathBuf> {
        Ok(server_root()?.join(format!("{prefix}-{}.{}", Uuid::new_v4(), extension)))
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

    fn assert_command_label(command: &Command, expected: &str) {
        assert_eq!(command_label(command), expected);
    }

    #[test]
    fn command_label_matches_core_variants() {
        assert_command_label(
            &Command::Torrent(TorrentCommand::Add(TorrentAddArgs {
                source: "magnet:?xt=urn:btih:demo".to_string(),
                name: None,
                id: None,
                storage_mode: None,
            })),
            "torrent_add",
        );
        assert_command_label(
            &Command::Action(TorrentActionArgs {
                id: Uuid::nil(),
                action: ActionType::Pause,
                enable: None,
                delete_data: false,
                download: None,
                upload: None,
                download_dir: None,
            }),
            "action_pause",
        );
        assert_command_label(
            &Command::Indexer(IndexerCommand::Import(IndexerImportCommand::Create(
                ImportJobCreateArgs {
                    source: ImportSourceArg::ProwlarrApi,
                    dry_run: false,
                    target_search_profile: None,
                    target_torznab_instance: None,
                },
            ))),
            "indexer_import_create",
        );
        assert_command_label(
            &Command::Indexer(IndexerCommand::Torznab(TorznabCommand::Rotate(
                TorznabRotateArgs {
                    torznab_instance_public_id: Uuid::nil(),
                },
            ))),
            "indexer_torznab_rotate",
        );
        assert_command_label(
            &Command::Indexer(IndexerCommand::Policy(Box::new(PolicyCommand::SetCreate(
                PolicySetCreateArgs {
                    display_name: "Demo".to_string(),
                    scope: "global".to_string(),
                    enabled: true,
                },
            )))),
            "indexer_policy_set_create",
        );
        assert_command_label(
            &Command::Indexer(IndexerCommand::Instance(Box::new(
                IndexerInstanceCommand::TestPrepare(IndexerInstanceTestPrepareArgs {
                    indexer_instance_public_id: Uuid::nil(),
                }),
            ))),
            "indexer_instance_test_prepare",
        );
    }

    #[test]
    fn command_label_matches_indexer_read_variants() {
        assert_command_label(
            &Command::Indexer(IndexerCommand::Read(Box::new(IndexerReadCommand::Tags))),
            "indexer_read_tags",
        );
        assert_command_label(
            &Command::Indexer(IndexerCommand::Read(Box::new(
                IndexerReadCommand::HealthNotifications,
            ))),
            "indexer_read_health_notifications",
        );
        assert_command_label(
            &Command::Indexer(IndexerCommand::Read(Box::new(
                IndexerReadCommand::RoutingPolicy(IndexerRoutingPolicyReadArgs {
                    routing_policy_public_id: Uuid::nil(),
                }),
            ))),
            "indexer_read_routing_policy",
        );
    }

    #[test]
    fn command_label_matches_indexer_write_variants() {
        assert_command_label(
            &Command::Indexer(IndexerCommand::Tag(Box::new(TagCommand::Create(
                TagCreateArgs {
                    tag_key: "anime".to_string(),
                    display_name: "Anime".to_string(),
                },
            )))),
            "indexer_tag_create",
        );
        assert_command_label(
            &Command::Indexer(IndexerCommand::Secret(Box::new(SecretCommand::Rotate(
                SecretRotateArgs {
                    secret_public_id: Uuid::nil(),
                    secret_value: "secret".to_string(),
                },
            )))),
            "indexer_secret_rotate",
        );
        assert_command_label(
            &Command::Indexer(IndexerCommand::HealthNotification(Box::new(
                HealthNotificationCommand::Update(HealthNotificationUpdateArgs {
                    indexer_health_notification_hook_public_id: Uuid::nil(),
                    display_name: Some("Ops Pager".to_string()),
                    status_threshold: None,
                    webhook_url: None,
                    email: None,
                    is_enabled: Some(true),
                }),
            ))),
            "indexer_health_notification_update",
        );
        assert_command_label(
            &Command::Indexer(IndexerCommand::CategoryMapping(Box::new(
                CategoryMappingCommand::TrackerUpsert(TrackerCategoryMappingUpsertArgs {
                    torznab_instance_public_id: None,
                    indexer_definition_upstream_slug: Some("demo".to_string()),
                    indexer_instance_public_id: None,
                    tracker_category: 2000,
                    tracker_subcategory: Some(10),
                    torznab_cat_id: 2010,
                    media_domain_key: Some("movies".to_string()),
                }),
            ))),
            "indexer_category_mapping_tracker_upsert",
        );
        assert_command_label(
            &Command::Indexer(IndexerCommand::RoutingPolicy(Box::new(
                RoutingPolicyCommand::Create(RoutingPolicyCreateArgs {
                    display_name: "Proxy".to_string(),
                    mode: "http_proxy".to_string(),
                }),
            ))),
            "indexer_routing_policy_create",
        );
        assert_command_label(
            &Command::Indexer(IndexerCommand::RateLimit(Box::new(
                RateLimitCommand::AssignInstance(RateLimitAssignInstanceArgs {
                    indexer_instance_public_id: Uuid::nil(),
                    rate_limit_policy_public_id: None,
                }),
            ))),
            "indexer_rate_limit_assign_instance",
        );
        assert_command_label(
            &Command::Indexer(IndexerCommand::SearchProfile(Box::new(
                SearchProfileCommand::SetTagPrefer(SearchProfileTagSetArgs {
                    search_profile_public_id: Uuid::nil(),
                    tag_public_ids: Vec::new(),
                    tag_keys: vec!["anime".to_string()],
                }),
            ))),
            "indexer_search_profile_set_tag_prefer",
        );
        assert_command_label(
            &Command::Indexer(IndexerCommand::Backup(Box::new(BackupCommand::Restore(
                BackupRestoreArgs {
                    file: PathBuf::from("/tmp/backup.json"),
                },
            )))),
            "indexer_backup_restore",
        );
        assert_command_label(
            &Command::Indexer(IndexerCommand::Rss(Box::new(RssCommand::MarkSeen(
                IndexerRssMarkSeenArgs {
                    indexer_instance_public_id: Uuid::nil(),
                    item_guid: Some("guid".to_string()),
                    infohash_v1: None,
                    infohash_v2: None,
                    magnet_hash: None,
                },
            )))),
            "indexer_rss_mark_seen",
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
            library_path: Some(".server_root/library/example".into()),
            download_dir: Some(".server_root/downloads/example".into()),
            sequential: false,
            tags: vec!["tag1".into()],
            category: None,
            trackers: vec!["https://tracker.example/announce".into()],
            rate_limit: None,
            connections_limit: None,
            added_at: now,
            completed_at: None,
            last_updated: now,
        }
    }

    fn sample_detail(id: Uuid, now: chrono::DateTime<Utc>) -> TorrentDetail {
        TorrentDetail {
            summary: sample_summary(id, now),
            settings: None,
            files: Some(vec![TorrentFileView {
                index: 0,
                path: "example.mkv".into(),
                size_bytes: 2_048,
                bytes_completed: 1_024,
                priority: revaer_torrent_core::FilePriority::High,
                selected: true,
            }]),
        }
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
    async fn run_with_cli_executes_indexer_read_health_notifications() -> Result<()> {
        let server = MockServer::start_async().await;
        let payload = serde_json::json!({
            "hooks": [{
                "indexer_health_notification_hook_public_id": Uuid::new_v4(),
                "channel": "webhook",
                "display_name": "Ops Pager",
                "status_threshold": "failing",
                "webhook_url": "https://hooks.example.test/revaer",
                "email": null,
                "is_enabled": true,
                "updated_at": "2026-04-03T00:00:00Z"
            }]
        });
        let hooks_mock = server.mock(|when, then| {
            when.method(GET)
                .path("/v1/indexers/health-notifications")
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
            "health-notifications",
        ]);

        let exit_code = run_with_cli(cli).await;
        hooks_mock.assert();
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

    #[tokio::test]
    async fn run_with_cli_executes_indexer_tag_create() -> Result<()> {
        let server = MockServer::start_async().await;
        let tag_public_id = Uuid::new_v4();
        let tag_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/indexers/tags")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(serde_json::json!({
                    "tag_key": "anime",
                    "display_name": "Anime"
                }));
            then.status(201).json_body(serde_json::json!({
                "tag_public_id": tag_public_id,
                "tag_key": "anime",
                "display_name": "Anime"
            }));
        });

        let cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "indexer",
            "tag",
            "create",
            "--tag-key",
            " anime ",
            "--display-name",
            " Anime ",
        ]);

        let exit_code = run_with_cli(cli).await;
        tag_mock.assert();
        assert_eq!(exit_code, 0);
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_indexer_secret_rotate() -> Result<()> {
        let server = MockServer::start_async().await;
        let secret_public_id = Uuid::new_v4();
        let secret_mock = server.mock(|when, then| {
            when.method(PATCH)
                .path("/v1/indexers/secrets")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(serde_json::json!({
                    "secret_public_id": secret_public_id,
                    "secret_value": "next-secret"
                }));
            then.status(200).json_body(serde_json::json!({
                "secret_public_id": secret_public_id
            }));
        });

        let cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "indexer",
            "secret",
            "rotate",
            "--secret-public-id",
            &secret_public_id.to_string(),
            "--secret-value",
            "next-secret",
        ]);

        let exit_code = run_with_cli(cli).await;
        secret_mock.assert();
        assert_eq!(exit_code, 0);
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_health_notification_update() -> Result<()> {
        let server = MockServer::start_async().await;
        let hook_public_id = Uuid::new_v4();
        let hook_mock = server.mock(|when, then| {
            when.method(PATCH)
                .path("/v1/indexers/health-notifications")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(serde_json::json!({
                    "indexer_health_notification_hook_public_id": hook_public_id,
                    "display_name": "Ops Pager",
                    "status_threshold": "quarantined",
                    "webhook_url": "https://hooks.example.test/revaer",
                    "is_enabled": true
                }));
            then.status(200).json_body(serde_json::json!({
                "indexer_health_notification_hook_public_id": hook_public_id,
                "channel": "webhook",
                "display_name": "Ops Pager",
                "status_threshold": "quarantined",
                "webhook_url": "https://hooks.example.test/revaer",
                "email": null,
                "is_enabled": true,
                "updated_at": "2026-04-03T00:00:00Z"
            }));
        });

        let cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "indexer",
            "health-notification",
            "update",
            &hook_public_id.to_string(),
            "--display-name",
            " Ops Pager ",
            "--status-threshold",
            " quarantined ",
            "--webhook-url",
            " https://hooks.example.test/revaer ",
            "--is-enabled",
            "true",
        ]);

        let exit_code = run_with_cli(cli).await;
        hook_mock.assert();
        assert_eq!(exit_code, 0);
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_tracker_category_mapping_upsert() -> Result<()> {
        let server = MockServer::start_async().await;
        let torznab_instance_public_id = Uuid::new_v4();
        let mapping_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/indexers/category-mappings/tracker")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(serde_json::json!({
                    "torznab_instance_public_id": torznab_instance_public_id,
                    "indexer_definition_upstream_slug": "demo",
                    "tracker_category": 2000,
                    "tracker_subcategory": 10,
                    "torznab_cat_id": 2010,
                    "media_domain_key": "movies"
                }));
            then.status(204);
        });

        let cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "indexer",
            "category-mapping",
            "tracker-upsert",
            "--torznab-instance-public-id",
            &torznab_instance_public_id.to_string(),
            "--indexer-definition-upstream-slug",
            " demo ",
            "--tracker-category",
            "2000",
            "--tracker-subcategory",
            "10",
            "--torznab-cat-id",
            "2010",
            "--media-domain-key",
            " movies ",
        ]);

        let exit_code = run_with_cli(cli).await;
        mapping_mock.assert();
        assert_eq!(exit_code, 0);
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_routing_policy_create() -> Result<()> {
        let server = MockServer::start_async().await;
        let routing_policy_public_id = Uuid::new_v4();
        let routing_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/indexers/routing-policies")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(serde_json::json!({
                    "display_name": "Proxy Lane",
                    "mode": "http_proxy"
                }));
            then.status(201).json_body(serde_json::json!({
                "routing_policy_public_id": routing_policy_public_id,
                "display_name": "Proxy Lane",
                "mode": "http_proxy"
            }));
        });

        let cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "indexer",
            "routing-policy",
            "create",
            "--display-name",
            " Proxy Lane ",
            "--mode",
            " http_proxy ",
        ]);

        let exit_code = run_with_cli(cli).await;
        routing_mock.assert();
        assert_eq!(exit_code, 0);
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_search_profile_set_default_domain() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        let profile_mock = server.mock(|when, then| {
            when.method(PUT)
                .path(format!(
                    "/v1/indexers/search-profiles/{search_profile_public_id}/default-domain"
                ))
                .header(HEADER_API_KEY, "key:secret")
                .json_body(serde_json::json!({
                    "default_media_domain_key": "movies"
                }));
            then.status(200).json_body(serde_json::json!({
                "search_profile_public_id": search_profile_public_id
            }));
        });

        let cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "indexer",
            "search-profile",
            "set-default-domain",
            &search_profile_public_id.to_string(),
            "--default-media-domain-key",
            " movies ",
        ]);

        let exit_code = run_with_cli(cli).await;
        profile_mock.assert();
        assert_eq!(exit_code, 0);
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_backup_restore() -> Result<()> {
        let server = MockServer::start_async().await;
        let backup_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/indexers/backup/restore")
                .header(HEADER_API_KEY, "key:secret");
            then.status(200).json_body(serde_json::json!({
                "created_tag_count": 1,
                "created_rate_limit_policy_count": 1,
                "created_routing_policy_count": 1,
                "created_indexer_instance_count": 1,
                "unresolved_secret_bindings": []
            }));
        });

        let snapshot_path =
            server_root()?.join(format!("indexer-backup-{}.json", Uuid::new_v4().simple()));
        fs::write(
            &snapshot_path,
            serde_json::to_vec(&serde_json::json!({
                "version": "v1",
                "exported_at": "2026-04-03T00:00:00Z",
                "tags": [],
                "rate_limit_policies": [],
                "routing_policies": [],
                "indexer_instances": [],
                "secrets": []
            }))?,
        )?;

        let cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "indexer",
            "backup",
            "restore",
            "--file",
            snapshot_path
                .to_str()
                .ok_or_else(|| anyhow!("invalid backup path"))?,
        ]);

        let exit_code = run_with_cli(cli).await;
        backup_mock.assert();
        assert_eq!(exit_code, 0);
        fs::remove_file(snapshot_path)?;
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_rss_mark_seen() -> Result<()> {
        let server = MockServer::start_async().await;
        let indexer_instance_public_id = Uuid::new_v4();
        let rss_mock = server.mock(|when, then| {
            when.method(POST)
                .path(format!(
                    "/v1/indexers/instances/{indexer_instance_public_id}/rss/items"
                ))
                .header(HEADER_API_KEY, "key:secret")
                .json_body(serde_json::json!({
                    "item_guid": "guid-1"
                }));
            then.status(200).json_body(serde_json::json!({
                "item": {
                    "item_guid": "guid-1",
                    "first_seen_at": "2026-04-03T00:00:00Z"
                },
                "inserted": true
            }));
        });

        let cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "indexer",
            "rss",
            "mark-seen",
            &indexer_instance_public_id.to_string(),
            "--item-guid",
            " guid-1 ",
        ]);

        let exit_code = run_with_cli(cli).await;
        rss_mock.assert();
        assert_eq!(exit_code, 0);
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_settings_patch_alias() -> Result<()> {
        let server = MockServer::start_async().await;
        let mock = server.mock(|when, then| {
            when.method(PATCH)
                .path("/v1/config")
                .header(HEADER_API_KEY, "key:secret");
            then.status(200);
        });

        let file_path = temp_path("revaer-cli-settings-patch", "json")?;
        let snapshot = sample_snapshot()?;
        fs::write(
            &file_path,
            serde_json::to_string(&serde_json::json!({
                "app_profile": snapshot.app_profile,
                "engine_profile": null,
                "fs_policy": null,
                "api_keys": [],
                "secrets": []
            }))?,
        )?;

        let cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "settings",
            "patch",
            "--file",
            file_path.to_str().ok_or_else(|| anyhow!("patch path utf8"))?,
        ]);

        let exit_code = run_with_cli(cli).await;
        mock.assert();
        fs::remove_file(&file_path)?;
        assert_eq!(exit_code, 0);
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_setup_start() -> Result<()> {
        let server = MockServer::start_async().await;
        let mock = server.mock(|when, then| {
            when.method(POST).path("/admin/setup/start");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({
                    "token": "token-1",
                    "expires_at": Utc::now().to_rfc3339()
                }));
        });

        let cli = Cli::parse_from(["revaer", "--api-url", &server.base_url(), "setup", "start"]);
        let exit_code = run_with_cli(cli).await;
        mock.assert();
        assert_eq!(exit_code, 0);
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_setup_complete() -> Result<()> {
        let server = MockServer::start_async().await;
        let snapshot = sample_snapshot()?;
        let completion_snapshot = snapshot.clone();
        server.mock(move |when, then| {
            when.method(GET).path("/.well-known/revaer.json");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!(snapshot));
        });
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/admin/setup/complete")
                .header(HEADER_SETUP_TOKEN, "token-1");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({
                    "snapshot": completion_snapshot,
                    "api_key": "admin:secret",
                    "api_key_expires_at": "2025-01-01T00:00:00Z"
                }));
        });

        let resume_dir = temp_path("revaer-cli-resume-dir", "d")?;
        let download_root = temp_path("revaer-cli-download-root", "d")?;
        let library_root = temp_path("revaer-cli-library-root", "d")?;
        fs::create_dir_all(&resume_dir)?;
        fs::create_dir_all(&download_root)?;
        fs::create_dir_all(&library_root)?;

        let cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "setup",
            "complete",
            "--token",
            "token-1",
            "--instance",
            "demo",
            "--bind",
            "127.0.0.1",
            "--port",
            "7070",
            "--resume-dir",
            resume_dir.to_str().ok_or_else(|| anyhow!("resume dir utf8"))?,
            "--download-root",
            download_root
                .to_str()
                .ok_or_else(|| anyhow!("download dir utf8"))?,
            "--library-root",
            library_root
                .to_str()
                .ok_or_else(|| anyhow!("library dir utf8"))?,
            "--api-key-label",
            "label",
            "--api-key-id",
            "admin",
            "--passphrase",
            "secret",
        ]);

        let exit_code = run_with_cli(cli).await;
        mock.assert();
        fs::remove_dir_all(&resume_dir)?;
        fs::remove_dir_all(&download_root)?;
        fs::remove_dir_all(&library_root)?;
        assert_eq!(exit_code, 0);
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_torrent_add_and_remove() -> Result<()> {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let add_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/torrents")
                .header(HEADER_API_KEY, "key:secret");
            then.status(202);
        });
        let remove_mock = server.mock(move |when, then| {
            when.method(POST)
                .path(format!("/v1/torrents/{torrent_id}/action").as_str())
                .header(HEADER_API_KEY, "key:secret");
            then.status(202);
        });

        let add_cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "torrent",
            "add",
            "--source",
            "magnet:?xt=urn:btih:demo",
        ]);
        let add_exit_code = run_with_cli(add_cli).await;
        add_mock.assert();
        assert_eq!(add_exit_code, 0);

        let remove_cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "torrent",
            "remove",
            &torrent_id.to_string(),
        ]);
        let remove_exit_code = run_with_cli(remove_cli).await;
        remove_mock.assert();
        assert_eq!(remove_exit_code, 0);
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_list_and_status_commands() -> Result<()> {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let now = Utc::now();
        let list_mock = server.mock(move |when, then| {
            when.method(GET).path("/v1/torrents");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({
                    "torrents": [sample_summary(torrent_id, now)],
                    "next": "cursor-1"
                }));
        });
        let detail_mock = server.mock(move |when, then| {
            when.method(GET)
                .path(format!("/v1/torrents/{torrent_id}").as_str());
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!(sample_detail(torrent_id, now)));
        });

        let list_cli = Cli::parse_from(["revaer", "--api-url", &server.base_url(), "ls"]);
        assert_eq!(run_with_cli(list_cli).await, 0);
        list_mock.assert();

        let status_cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "status",
            &torrent_id.to_string(),
        ]);
        assert_eq!(run_with_cli(status_cli).await, 0);
        detail_mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn run_with_cli_executes_select_action_and_tail() -> Result<()> {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let select_mock = server.mock(move |when, then| {
            when.method(POST)
                .path(format!("/v1/torrents/{torrent_id}/select").as_str())
                .header(HEADER_API_KEY, "key:secret");
            then.status(200);
        });
        let action_mock = server.mock(move |when, then| {
            when.method(POST)
                .path(format!("/v1/torrents/{torrent_id}/action").as_str())
                .header(HEADER_API_KEY, "key:secret");
            then.status(202);
        });
        let event = EventEnvelope {
            id: 3,
            timestamp: Utc::now(),
            event: Event::TorrentRemoved { torrent_id },
        };
        let payload = serde_json::to_string(&event)?;
        server.mock(move |when, then| {
            when.method(GET).path("/v1/torrents/events");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(format!("id:3\ndata:{payload}\n\n"));
        });

        let select_cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "select",
            &torrent_id.to_string(),
            "--include",
            "**/*.mkv",
        ]);
        assert_eq!(run_with_cli(select_cli).await, 0);
        select_mock.assert();

        let action_cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "action",
            &torrent_id.to_string(),
            "sequential",
            "--enable",
            "true",
        ]);
        assert_eq!(run_with_cli(action_cli).await, 0);
        action_mock.assert();

        let resume_path = temp_path("revaer-cli-tail-resume", "txt")?;
        let tail_cli = Cli::parse_from([
            "revaer",
            "--api-url",
            &server.base_url(),
            "--api-key",
            "key:secret",
            "tail",
            "--resume-file",
            resume_path
                .to_str()
                .ok_or_else(|| anyhow!("resume path utf8"))?,
            "--retry-secs",
            "0",
        ]);
        let tail_result = timeout(Duration::from_millis(200), run_with_cli(tail_cli)).await;
        assert!(tail_result.is_err(), "tail should be cancelled by timeout");
        let saved = fs::read_to_string(&resume_path)?;
        assert_eq!(saved.trim(), "3");
        fs::remove_file(&resume_path)?;
        Ok(())
    }
}
