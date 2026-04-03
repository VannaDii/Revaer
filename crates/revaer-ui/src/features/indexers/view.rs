//! Indexer administration page.
//!
//! # Design
//! - Surface ERD-backed indexer admin operations that already exist on the API.
//! - Keep interaction local to the page with explicit operator-triggered actions.
//! - Preserve a lightweight activity log so responses remain visible after toasts dismiss.

use crate::app::api::ApiCtx;
use crate::components::atoms::EmptyState;
use crate::components::daisy::{Button, Input, Textarea};
use crate::features::indexers::api::{
    add_search_profile_policy_set, assign_rate_limit_to_indexer, assign_rate_limit_to_routing,
    bind_indexer_field_secret, bind_routing_secret, create_health_notification_hook,
    create_import_job, create_indexer_instance, create_policy_rule, create_policy_set,
    create_rate_limit_policy, create_routing_policy, create_search_profile, create_secret,
    create_tag, create_torznab_instance, delete_health_notification_hook, delete_rate_limit_policy,
    delete_tag, delete_torznab_instance, delete_tracker_category_mapping, export_backup_snapshot,
    fetch_cf_state, fetch_definitions, fetch_health_notification_hooks, fetch_import_job_results,
    fetch_import_job_status, fetch_indexer_connectivity_profile, fetch_indexer_health_events,
    fetch_indexer_instances, fetch_indexer_rss_items, fetch_indexer_rss_subscription,
    fetch_indexer_source_reputation, fetch_rate_limit_policies, fetch_routing_policies,
    fetch_routing_policy, fetch_secret_metadata, fetch_source_metadata_conflicts, fetch_tags,
    finalize_indexer_test, import_cardigann_definition, mark_indexer_rss_item_seen,
    prepare_indexer_test, provision_app_sync, reopen_source_metadata_conflict, reset_cf_state,
    resolve_source_metadata_conflict, restore_backup_snapshot, revoke_secret, rotate_secret,
    rotate_torznab_key, run_import_job_api, run_import_job_backup, set_indexer_field_value,
    set_indexer_media_domains, set_indexer_tags, set_routing_param,
    set_search_profile_default_domain, set_search_profile_indexers,
    set_search_profile_media_domains, set_search_profile_tags, set_torznab_state,
    update_health_notification_hook, update_indexer_instance, update_indexer_rss_subscription,
    update_rate_limit_policy, update_search_profile, update_tag, upsert_tracker_category_mapping,
};
use crate::features::indexers::logic::{
    append_csv_unique, connectivity_status_badge_class, filtered_definitions,
    format_optional_percent,
};
use crate::features::indexers::state::{
    AppSyncProvisionSummary, AppSyncState, BackupState, CardigannImportState,
    ConnectivityInsightsState, DefinitionsState, HealthEventsState, HealthNotificationHooksState,
    ImportJobState, IndexerInstanceInventoryState, IndexersDraft, OperationRecord,
    RateLimitInventoryState, RoutingPolicyInventoryState, RoutingPolicyState, SecretInventoryState,
    SourceMetadataConflictsState, TagInventoryState,
};
use crate::models::{
    CardigannDefinitionImportResponse, ImportJobResultResponse, ImportJobStatusResponse,
    IndexerBackupSnapshot, IndexerBackupUnresolvedSecretBinding,
    IndexerConnectivityProfileResponse, IndexerDefinitionResponse, IndexerHealthEventResponse,
    IndexerHealthNotificationHookResponse, IndexerInstanceListItemResponse,
    IndexerSourceMetadataConflictResponse, IndexerSourceReputationResponse,
    RateLimitPolicyListItemResponse, RoutingPolicyDetailResponse, RoutingPolicyListItemResponse,
    SecretMetadataResponse, TagListItemResponse,
};
use serde::Serialize;
use std::future::Future;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::platform::spawn_local;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct IndexersPageProps {
    pub on_success_toast: Callback<String>,
    pub on_error_toast: Callback<String>,
}

#[derive(Properties, PartialEq)]
struct DefinitionsSectionProps {
    draft: UseStateHandle<IndexersDraft>,
    definitions: DefinitionsState,
    cardigann_import: CardigannImportState,
    busy: bool,
    on_refresh: Callback<MouseEvent>,
    on_import_cardigann: Callback<MouseEvent>,
}

#[derive(Properties, PartialEq)]
struct TagSectionProps {
    draft: UseStateHandle<IndexersDraft>,
    tags: TagInventoryState,
    busy: bool,
    on_fetch_tags: Callback<MouseEvent>,
    on_primary: Callback<MouseEvent>,
    on_secondary: Callback<MouseEvent>,
    on_tertiary: Callback<MouseEvent>,
}

#[derive(Properties, PartialEq)]
struct SecretSectionProps {
    draft: UseStateHandle<IndexersDraft>,
    secrets: SecretInventoryState,
    busy: bool,
    on_fetch_secrets: Callback<MouseEvent>,
    on_primary: Callback<MouseEvent>,
    on_secondary: Callback<MouseEvent>,
    on_tertiary: Callback<MouseEvent>,
}

#[derive(Properties, PartialEq)]
struct ProfilesPoliciesSectionProps {
    draft: UseStateHandle<IndexersDraft>,
    busy: bool,
    on_create_profile: Callback<MouseEvent>,
    on_update_profile: Callback<MouseEvent>,
    on_default_domain: Callback<MouseEvent>,
    on_media_domains: Callback<MouseEvent>,
    on_add_policy_set: Callback<MouseEvent>,
    on_allow_indexers: Callback<MouseEvent>,
    on_block_indexers: Callback<MouseEvent>,
    on_allow_tags: Callback<MouseEvent>,
    on_block_tags: Callback<MouseEvent>,
    on_prefer_tags: Callback<MouseEvent>,
    on_create_policy_set: Callback<MouseEvent>,
    on_create_policy_rule: Callback<MouseEvent>,
}

#[derive(Properties, PartialEq)]
struct ImportTorznabSectionProps {
    draft: UseStateHandle<IndexersDraft>,
    app_sync: AppSyncState,
    import_job: ImportJobState,
    source_conflicts: SourceMetadataConflictsState,
    backup: BackupState,
    busy: bool,
    on_provision_app_sync: Callback<MouseEvent>,
    on_create_import_job: Callback<MouseEvent>,
    on_run_import_api: Callback<MouseEvent>,
    on_run_import_backup: Callback<MouseEvent>,
    on_fetch_import_status: Callback<MouseEvent>,
    on_fetch_import_results: Callback<MouseEvent>,
    on_fetch_source_conflicts: Callback<MouseEvent>,
    on_resolve_source_conflict: Callback<MouseEvent>,
    on_reopen_source_conflict: Callback<MouseEvent>,
    on_export_backup: Callback<MouseEvent>,
    on_restore_backup: Callback<MouseEvent>,
    on_create_torznab: Callback<MouseEvent>,
    on_rotate_torznab: Callback<MouseEvent>,
    on_set_torznab_state: Callback<MouseEvent>,
    on_delete_torznab: Callback<MouseEvent>,
}

#[derive(Properties, PartialEq)]
struct HealthNotificationsSectionProps {
    draft: UseStateHandle<IndexersDraft>,
    hooks: HealthNotificationHooksState,
    busy: bool,
    on_fetch_hooks: Callback<MouseEvent>,
    on_create_hook: Callback<MouseEvent>,
    on_update_hook: Callback<MouseEvent>,
    on_delete_hook: Callback<MouseEvent>,
}

#[derive(Properties, PartialEq)]
struct ActivityLogProps {
    records: Vec<OperationRecord>,
}

fn append_record(
    records: &UseStateHandle<Vec<OperationRecord>>,
    title: impl Into<String>,
    body: impl Into<String>,
) {
    let mut next = (**records).clone();
    next.insert(
        0,
        OperationRecord {
            title: title.into(),
            body: body.into(),
        },
    );
    records.set(next);
}

fn append_json_record<T: Serialize>(
    records: &UseStateHandle<Vec<OperationRecord>>,
    title: impl Into<String>,
    value: &T,
) {
    let body = serde_json::to_string_pretty(value)
        .unwrap_or_else(|_| "Failed to encode response".to_string());
    append_record(records, title, body);
}

fn text_callback<F>(draft: UseStateHandle<IndexersDraft>, update: F) -> Callback<String>
where
    F: Fn(&mut IndexersDraft, String) + 'static,
{
    Callback::from(move |value: String| {
        let mut next = (*draft).clone();
        update(&mut next, value);
        draft.set(next);
    })
}

fn bool_callback<F>(draft: UseStateHandle<IndexersDraft>, update: F) -> Callback<Event>
where
    F: Fn(&mut IndexersDraft, bool) + 'static,
{
    Callback::from(move |event: Event| {
        let Some(target) = event.target() else {
            return;
        };
        let Ok(input) = target.dyn_into::<HtmlInputElement>() else {
            return;
        };
        let mut next = (*draft).clone();
        update(&mut next, input.checked());
        draft.set(next);
    })
}

fn action_callback<T, F, Fut>(
    api: Option<ApiCtx>,
    busy: UseStateHandle<bool>,
    records: UseStateHandle<Vec<OperationRecord>>,
    on_success_toast: Callback<String>,
    on_error_toast: Callback<String>,
    title: &'static str,
    success_message: &'static str,
    operation: F,
) -> Callback<MouseEvent>
where
    T: Serialize + 'static,
    F: Fn(ApiCtx) -> Fut + Clone + 'static,
    Fut: Future<Output = Result<T, String>> + 'static,
{
    Callback::from(move |_| {
        let Some(api) = api.clone() else {
            append_record(&records, title, "API context is unavailable");
            on_error_toast.emit("Indexer API context is unavailable".to_string());
            return;
        };
        if *busy {
            return;
        }
        busy.set(true);
        let busy = busy.clone();
        let records = records.clone();
        let on_success_toast = on_success_toast.clone();
        let on_error_toast = on_error_toast.clone();
        let operation = operation.clone();
        spawn_local(async move {
            match operation(api).await {
                Ok(value) => {
                    append_json_record(&records, title, &value);
                    on_success_toast.emit(success_message.to_string());
                }
                Err(error) => {
                    append_record(&records, title, error.clone());
                    on_error_toast.emit(format!("{title}: {error}"));
                }
            }
            busy.set(false);
        });
    })
}

fn void_action_callback<F, Fut>(
    api: Option<ApiCtx>,
    busy: UseStateHandle<bool>,
    records: UseStateHandle<Vec<OperationRecord>>,
    on_success_toast: Callback<String>,
    on_error_toast: Callback<String>,
    title: &'static str,
    success_message: &'static str,
    operation: F,
) -> Callback<MouseEvent>
where
    F: Fn(ApiCtx) -> Fut + Clone + 'static,
    Fut: Future<Output = Result<(), String>> + 'static,
{
    Callback::from(move |_| {
        let Some(api) = api.clone() else {
            append_record(&records, title, "API context is unavailable");
            on_error_toast.emit("Indexer API context is unavailable".to_string());
            return;
        };
        if *busy {
            return;
        }
        busy.set(true);
        let busy = busy.clone();
        let records = records.clone();
        let on_success_toast = on_success_toast.clone();
        let on_error_toast = on_error_toast.clone();
        let operation = operation.clone();
        spawn_local(async move {
            match operation(api).await {
                Ok(()) => {
                    append_record(&records, title, success_message);
                    on_success_toast.emit(success_message.to_string());
                }
                Err(error) => {
                    append_record(&records, title, error.clone());
                    on_error_toast.emit(format!("{title}: {error}"));
                }
            }
            busy.set(false);
        });
    })
}

fn field(label: &str, hint: &str, control: Html) -> Html {
    html! {
        <label class="form-control gap-2">
            <span class="label-text font-medium">{label}</span>
            {control}
            <span class="text-xs text-base-content/60">{hint}</span>
        </label>
    }
}

fn card(title: &str, body: Html) -> Html {
    html! {
        <section class="rounded-box border border-base-300 bg-base-100 p-4 shadow-sm">
            <div class="mb-4 flex items-start justify-between gap-3">
                <div>
                    <h2 class="text-lg font-semibold">{title}</h2>
                </div>
            </div>
            {body}
        </section>
    }
}

fn render_routing_policy_detail(detail: &RoutingPolicyDetailResponse) -> Html {
    let rate_limit = detail.rate_limit_display_name.as_ref().map_or_else(
        || "No rate-limit policy assigned.".to_string(),
        |_| {
            format!(
                "{} ({:?} rpm / {:?} burst / {:?} concurrent)",
                detail.rate_limit_display_name.as_deref().unwrap_or(""),
                detail.rate_limit_requests_per_minute,
                detail.rate_limit_burst,
                detail.rate_limit_concurrent_requests
            )
        },
    );

    html! {
        <div class="space-y-3 rounded-box border border-base-300 bg-base-200/40 p-4">
            <div class="flex flex-wrap items-center justify-between gap-2">
                <div>
                    <h3 class="font-semibold">{detail.display_name.clone()}</h3>
                    <p class="text-sm text-base-content/70">{format!("Mode: {}", detail.mode)}</p>
                </div>
                <code class="text-xs">{detail.routing_policy_public_id.to_string()}</code>
            </div>
            <p class="text-sm text-base-content/80">{rate_limit}</p>
            <div class="space-y-2">
                <h4 class="text-sm font-medium">{"Parameters"}</h4>
                if detail.parameters.is_empty() {
                    <p class="text-sm text-base-content/60">{"No routing parameters stored yet."}</p>
                } else {
                    {for detail.parameters.iter().map(|parameter| {
                        let mut fragments = Vec::new();
                        if let Some(value_plain) = parameter.value_plain.as_deref() {
                            fragments.push(format!("plain={value_plain}"));
                        }
                        if let Some(value_int) = parameter.value_int {
                            fragments.push(format!("int={value_int}"));
                        }
                        if let Some(value_bool) = parameter.value_bool {
                            fragments.push(format!("bool={value_bool}"));
                        }
                        if let Some(secret_public_id) = parameter.secret_public_id {
                            fragments.push(format!("secret={secret_public_id}"));
                        }
                        if let Some(secret_binding_name) = parameter.secret_binding_name.as_deref() {
                            fragments.push(format!("binding={secret_binding_name}"));
                        }
                        let value = if fragments.is_empty() {
                            "configured with no visible value".to_string()
                        } else {
                            fragments.join(" | ")
                        };
                        html! {
                            <div class="rounded-box border border-base-300 bg-base-100 px-3 py-2 text-sm">
                                <div class="font-mono text-xs uppercase tracking-wide text-base-content/60">
                                    {parameter.param_key.clone()}
                                </div>
                                <div>{value}</div>
                            </div>
                        }
                    })}
                }
            </div>
        </div>
    }
}

fn render_import_status(status: &ImportJobStatusResponse) -> Html {
    html! {
        <div class="grid gap-3 md:grid-cols-3">
            <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
                <div class="text-xs uppercase tracking-wide text-base-content/60">{"Job status"}</div>
                <div class="text-lg font-semibold">{status.status.clone()}</div>
            </div>
            <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
                <div class="text-xs uppercase tracking-wide text-base-content/60">{"Total results"}</div>
                <div class="text-lg font-semibold">{status.result_total}</div>
            </div>
            <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
                <div class="text-xs uppercase tracking-wide text-base-content/60">{"Imported ready"}</div>
                <div class="text-lg font-semibold">{status.result_imported_ready}</div>
            </div>
            <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
                <div class="text-xs uppercase tracking-wide text-base-content/60">{"Needs secret"}</div>
                <div class="text-lg font-semibold">{status.result_imported_needs_secret}</div>
            </div>
            <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
                <div class="text-xs uppercase tracking-wide text-base-content/60">{"Test failed"}</div>
                <div class="text-lg font-semibold">{status.result_imported_test_failed}</div>
            </div>
            <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
                <div class="text-xs uppercase tracking-wide text-base-content/60">{"Unmapped / duplicate"}</div>
                <div class="text-lg font-semibold">
                    {format!(
                        "{} / {}",
                        status.result_unmapped_definition, status.result_skipped_duplicate
                    )}
                </div>
            </div>
        </div>
    }
}

fn render_import_result(result: &ImportJobResultResponse) -> Html {
    let detail = result
        .detail
        .as_deref()
        .map_or_else(|| "No detail returned.".to_string(), str::to_string);
    let domains = if result.media_domain_keys.is_empty() {
        "none".to_string()
    } else {
        result.media_domain_keys.join(", ")
    };
    let tags = if result.tag_keys.is_empty() {
        "none".to_string()
    } else {
        result.tag_keys.join(", ")
    };

    html! {
        <div class="rounded-box border border-base-300 bg-base-100 p-4">
            <div class="flex flex-wrap items-start justify-between gap-2">
                <div>
                    <div class="font-semibold">{result.prowlarr_identifier.clone()}</div>
                    <div class="text-sm text-base-content/70">{result.upstream_slug.clone().unwrap_or_else(|| "definition unavailable".to_string())}</div>
                </div>
                <span class="badge badge-outline">{result.status.clone()}</span>
            </div>
            <div class="mt-3 space-y-1 text-sm">
                <div>{detail}</div>
                <div>{format!("Missing secret fields: {}", result.missing_secret_fields)}</div>
                <div>{format!("Media domains: {domains}")}</div>
                <div>{format!("Tags: {tags}")}</div>
                if let Some(indexer_instance_public_id) = result.indexer_instance_public_id {
                    <div>{format!("Indexer instance: {indexer_instance_public_id}")}</div>
                }
                if let Some(priority) = result.resolved_priority {
                    <div>{format!("Resolved priority: {priority}")}</div>
                }
                if let Some(is_enabled) = result.resolved_is_enabled {
                    <div>{format!("Resolved enabled: {is_enabled}")}</div>
                }
            </div>
        </div>
    }
}

fn render_source_metadata_conflict(result: &IndexerSourceMetadataConflictResponse) -> Html {
    let resolution = result
        .resolution
        .clone()
        .unwrap_or_else(|| "unresolved".to_string());
    let note = result
        .resolution_note
        .clone()
        .unwrap_or_else(|| "No note recorded.".to_string());

    html! {
        <div class="rounded-box border border-base-300 bg-base-100 p-4">
            <div class="flex flex-wrap items-start justify-between gap-2">
                <div>
                    <div class="font-semibold">{format!("Conflict #{}", result.conflict_id)}</div>
                    <div class="text-sm text-base-content/70">{result.conflict_type.clone()}</div>
                </div>
                <span class={classes!(
                    "badge",
                    if result.resolved_at.is_some() { "badge-outline" } else { "badge-warning" }
                )}>
                    {resolution}
                </span>
            </div>
            <div class="mt-3 space-y-1 text-sm">
                <div>{format!("Existing: {}", result.existing_value)}</div>
                <div>{format!("Incoming: {}", result.incoming_value)}</div>
                <div>{format!("Observed: {}", result.observed_at)}</div>
                if let Some(resolved_at) = result.resolved_at {
                    <div>{format!("Resolved: {resolved_at}")}</div>
                }
                <div>{note}</div>
            </div>
        </div>
    }
}

fn render_app_sync_summary(summary: &AppSyncProvisionSummary) -> Html {
    let default_domain = summary
        .default_media_domain_key
        .clone()
        .unwrap_or_else(|| "none".to_string());
    let media_domains = if summary.media_domain_keys.is_empty() {
        "none".to_string()
    } else {
        summary.media_domain_keys.join(", ")
    };
    let allowed_indexers = if summary.allowed_indexer_public_ids.is_empty() {
        "none".to_string()
    } else {
        summary
            .allowed_indexer_public_ids
            .iter()
            .map(uuid::Uuid::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let allow_tags = if summary.allowed_tag_keys.is_empty() {
        "none".to_string()
    } else {
        summary.allowed_tag_keys.join(", ")
    };
    let block_tags = if summary.blocked_tag_keys.is_empty() {
        "none".to_string()
    } else {
        summary.blocked_tag_keys.join(", ")
    };
    let prefer_tags = if summary.preferred_tag_keys.is_empty() {
        "none".to_string()
    } else {
        summary.preferred_tag_keys.join(", ")
    };

    html! {
        <div class="rounded-box border border-base-300 bg-base-100 p-4">
            <div class="flex flex-wrap items-start justify-between gap-2">
                <div>
                    <div class="font-semibold">{"Provisioned downstream app sync"}</div>
                    <div class="text-sm text-base-content/70">
                        {
                            if summary.created_search_profile {
                                "Created a new search profile and Torznab endpoint."
                            } else {
                                "Reused the selected search profile and created a new Torznab endpoint."
                            }
                        }
                    </div>
                </div>
                <span class="badge badge-outline">{"ready"}</span>
            </div>
            <div class="mt-3 space-y-1 text-sm">
                <div>{format!("Search profile: {}", summary.search_profile_public_id)}</div>
                <div>{format!("Torznab instance: {}", summary.torznab_instance_public_id)}</div>
                <div class="font-mono text-xs break-all">
                    {format!("API key: {}", summary.torznab_api_key_plaintext)}
                </div>
                <div>{format!("Default domain: {default_domain}")}</div>
                <div>{format!("Media domains: {media_domains}")}</div>
                <div>{format!("Allowed indexers: {allowed_indexers}")}</div>
                <div>{format!("Allow tags: {allow_tags}")}</div>
                <div>{format!("Block tags: {block_tags}")}</div>
                <div>{format!("Prefer tags: {prefer_tags}")}</div>
            </div>
        </div>
    }
}

fn render_backup_snapshot(snapshot: &IndexerBackupSnapshot) -> Html {
    html! {
        <div class="grid gap-3 md:grid-cols-3">
            <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
                <div class="text-xs uppercase tracking-wide text-base-content/60">{"Version"}</div>
                <div class="text-sm font-semibold">{snapshot.version.clone()}</div>
            </div>
            <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
                <div class="text-xs uppercase tracking-wide text-base-content/60">{"Exported tags"}</div>
                <div class="text-lg font-semibold">{snapshot.tags.len()}</div>
            </div>
            <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
                <div class="text-xs uppercase tracking-wide text-base-content/60">{"Exported indexers"}</div>
                <div class="text-lg font-semibold">{snapshot.indexer_instances.len()}</div>
            </div>
            <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
                <div class="text-xs uppercase tracking-wide text-base-content/60">{"Routing policies"}</div>
                <div class="text-lg font-semibold">{snapshot.routing_policies.len()}</div>
            </div>
            <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
                <div class="text-xs uppercase tracking-wide text-base-content/60">{"Rate limits"}</div>
                <div class="text-lg font-semibold">{snapshot.rate_limit_policies.len()}</div>
            </div>
            <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
                <div class="text-xs uppercase tracking-wide text-base-content/60">{"Secret refs"}</div>
                <div class="text-lg font-semibold">{snapshot.secrets.len()}</div>
            </div>
        </div>
    }
}

fn render_unresolved_secret_binding(binding: &IndexerBackupUnresolvedSecretBinding) -> Html {
    html! {
        <div class="rounded-box border border-warning/40 bg-warning/10 p-3 text-sm">
            <div class="font-semibold">{format!("{}: {}", binding.entity_type, binding.entity_display_name)}</div>
            <div>{format!("Missing binding for {}", binding.binding_key)}</div>
            <div class="font-mono text-xs text-base-content/70">{binding.secret_public_id.to_string()}</div>
        </div>
    }
}

fn render_health_event(event: &IndexerHealthEventResponse) -> Html {
    let mut metadata = Vec::new();
    if let Some(error_class) = event.error_class.as_deref() {
        metadata.push(format!("error={error_class}"));
    }
    if let Some(http_status) = event.http_status {
        metadata.push(format!("http={http_status}"));
    }
    if let Some(latency_ms) = event.latency_ms {
        metadata.push(format!("latency={latency_ms}ms"));
    }

    html! {
        <div class="space-y-2 rounded-box border border-base-300 bg-base-200/40 p-3">
            <div class="flex flex-wrap items-center justify-between gap-2">
                <div>
                    <div class="text-xs uppercase tracking-wide text-base-content/60">
                        {event.event_type.clone()}
                    </div>
                    <div class="font-medium">{event.occurred_at.to_rfc3339()}</div>
                </div>
                if metadata.is_empty() {
                    <span class="text-sm text-base-content/60">{"No HTTP or error metadata."}</span>
                } else {
                    <span class="text-sm text-base-content/80">{metadata.join(" | ")}</span>
                }
            </div>
            if let Some(detail) = event.detail.as_deref() {
                <p class="text-sm text-base-content/80">{detail}</p>
            }
        </div>
    }
}

fn render_health_notification_hook(hook: &IndexerHealthNotificationHookResponse) -> Html {
    let destination = match (
        hook.channel.as_str(),
        hook.webhook_url.as_deref(),
        hook.email.as_deref(),
    ) {
        ("webhook", Some(webhook_url), _) => webhook_url.to_string(),
        ("email", _, Some(email)) => email.to_string(),
        _ => "n/a".to_string(),
    };
    let enabled_badge = if hook.is_enabled {
        "badge badge-success badge-outline"
    } else {
        "badge badge-ghost"
    };

    html! {
        <div class="rounded-box border border-base-300 bg-base-200/40 p-4">
            <div class="flex flex-wrap items-start justify-between gap-2">
                <div>
                    <div class="font-semibold">{hook.display_name.clone()}</div>
                    <div class="text-sm text-base-content/70">{destination}</div>
                </div>
                <div class="flex flex-wrap gap-2">
                    <span class="badge badge-outline">{hook.channel.clone()}</span>
                    <span class="badge badge-outline">{format!("trigger {}", hook.status_threshold)}</span>
                    <span class={enabled_badge}>{if hook.is_enabled { "enabled" } else { "disabled" }}</span>
                </div>
            </div>
            <div class="mt-3 text-xs font-mono text-base-content/60">
                {hook.indexer_health_notification_hook_public_id.to_string()}
            </div>
        </div>
    }
}

fn render_connectivity_profile(profile: &IndexerConnectivityProfileResponse) -> Html {
    if !profile.profile_exists {
        return html! {
            <div class="rounded-box border border-dashed border-base-300 bg-base-200/40 p-4 text-sm text-base-content/70">
                {"No derived connectivity profile exists yet for this indexer instance."}
            </div>
        };
    }

    let status = profile.status.as_deref();
    let error_class = profile
        .error_class
        .as_deref()
        .map_or_else(|| "none".to_string(), str::to_string);
    let checked_at = profile
        .last_checked_at
        .map_or_else(|| "n/a".to_string(), |timestamp| timestamp.to_rfc3339());

    html! {
        <div class="space-y-3 rounded-box border border-base-300 bg-base-200/40 p-4">
            <div class="flex flex-wrap items-center justify-between gap-3">
                <div>
                    <h4 class="font-semibold">{"Connectivity profile"}</h4>
                    <p class="text-sm text-base-content/70">{format!("Last refresh: {checked_at}")}</p>
                </div>
                <span class={connectivity_status_badge_class(status)}>
                    {status.unwrap_or("missing")}
                </span>
            </div>
            <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                <div class="rounded-box border border-base-300 bg-base-100 p-3">
                    <div class="text-xs uppercase tracking-wide text-base-content/60">{"Dominant error"}</div>
                    <div class="font-medium">{error_class}</div>
                </div>
                <div class="rounded-box border border-base-300 bg-base-100 p-3">
                    <div class="text-xs uppercase tracking-wide text-base-content/60">{"Latency p50 / p95"}</div>
                    <div class="font-medium">
                        {format!(
                            "{} / {} ms",
                            profile
                                .latency_p50_ms
                                .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
                            profile
                                .latency_p95_ms
                                .map_or_else(|| "n/a".to_string(), |value| value.to_string()),
                        )}
                    </div>
                </div>
                <div class="rounded-box border border-base-300 bg-base-100 p-3">
                    <div class="text-xs uppercase tracking-wide text-base-content/60">{"Success rate 1h / 24h"}</div>
                    <div class="font-medium">
                        {format!(
                            "{} / {}",
                            format_optional_percent(profile.success_rate_1h),
                            format_optional_percent(profile.success_rate_24h),
                        )}
                    </div>
                </div>
            </div>
        </div>
    }
}

fn render_source_reputation(item: &IndexerSourceReputationResponse) -> Html {
    html! {
        <div class="rounded-box border border-base-300 bg-base-100 p-4">
            <div class="flex flex-wrap items-center justify-between gap-2">
                <div>
                    <div class="font-semibold">{item.window_key.clone()}</div>
                    <div class="text-sm text-base-content/70">
                        {format!("Window start: {}", item.window_start.to_rfc3339())}
                    </div>
                </div>
                <span class="badge badge-outline">{format!("min samples {}", item.min_samples)}</span>
            </div>
            <div class="mt-3 grid gap-3 md:grid-cols-2 xl:grid-cols-3 text-sm">
                <div>{format!("Request success: {}", format_optional_percent(Some(item.request_success_rate)))}</div>
                <div>{format!("Acquisition success: {}", format_optional_percent(Some(item.acquisition_success_rate)))}</div>
                <div>{format!("Fake rate: {}", format_optional_percent(Some(item.fake_rate)))}</div>
                <div>{format!("DMCA rate: {}", format_optional_percent(Some(item.dmca_rate)))}</div>
                <div>{format!("Requests: {} / successes {}", item.request_count, item.request_success_count)}</div>
                <div>{format!("Acquisitions: {} / successes {}", item.acquisition_count, item.acquisition_success_count)}</div>
            </div>
        </div>
    }
}

#[function_component(IndexersPage)]
pub(crate) fn indexers_page(props: &IndexersPageProps) -> Html {
    let api = use_context::<ApiCtx>();
    let draft = use_state(IndexersDraft::default);
    let definitions = use_state(DefinitionsState::default);
    let cardigann_import = use_state(CardigannImportState::default);
    let connectivity = use_state(ConnectivityInsightsState::default);
    let health_events = use_state(HealthEventsState::default);
    let health_notification_hooks = use_state(HealthNotificationHooksState::default);
    let tag_inventory = use_state(TagInventoryState::default);
    let secret_inventory = use_state(SecretInventoryState::default);
    let routing_inventory = use_state(RoutingPolicyInventoryState::default);
    let rate_limit_inventory = use_state(RateLimitInventoryState::default);
    let indexer_instance_inventory = use_state(IndexerInstanceInventoryState::default);
    let app_sync = use_state(AppSyncState::default);
    let import_job = use_state(ImportJobState::default);
    let source_conflicts = use_state(SourceMetadataConflictsState::default);
    let backup = use_state(BackupState::default);
    let routing_policy = use_state(RoutingPolicyState::default);
    let busy = use_state(|| false);
    let records = use_state(Vec::<OperationRecord>::new);
    let definitions_busy = use_state(|| false);

    {
        let api = api.clone();
        let definitions = definitions.clone();
        let records = records.clone();
        let on_error_toast = props.on_error_toast.clone();
        let definitions_busy = definitions_busy.clone();
        use_effect_with((), move |_| {
            let Some(api) = api.clone() else {
                append_record(&records, "Definitions", "API context is unavailable");
                return;
            };
            definitions_busy.set(true);
            spawn_local(async move {
                let client = api.client.clone();
                match fetch_definitions(&client).await {
                    Ok(response) => {
                        definitions.set(DefinitionsState {
                            definitions: response.definitions,
                            loaded: true,
                        });
                    }
                    Err(error) => {
                        append_record(&records, "Definitions", error.to_string());
                        on_error_toast.emit(format!("Definitions: {error}"));
                    }
                }
                definitions_busy.set(false);
            });
        });
    }

    let on_refresh_definitions = {
        let api = api.clone();
        let definitions = definitions.clone();
        let records = records.clone();
        let definitions_busy = definitions_busy.clone();
        let on_error_toast = props.on_error_toast.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(&records, "Definitions", "API context is unavailable");
                on_error_toast.emit("Definitions: API context is unavailable".to_string());
                return;
            };
            if *definitions_busy {
                return;
            }
            definitions_busy.set(true);
            let definitions = definitions.clone();
            let records = records.clone();
            let definitions_busy = definitions_busy.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                let client = api.client.clone();
                match fetch_definitions(&client).await {
                    Ok(response) => {
                        append_json_record(&records, "Definitions", &response);
                        definitions.set(DefinitionsState {
                            definitions: response.definitions,
                            loaded: true,
                        });
                    }
                    Err(error) => {
                        append_record(&records, "Definitions", error.to_string());
                        on_error_toast.emit(format!("Definitions: {error}"));
                    }
                }
                definitions_busy.set(false);
            });
        })
    };

    let on_import_cardigann = {
        let api = api.clone();
        let draft = draft.clone();
        let definitions = definitions.clone();
        let cardigann_import = cardigann_import.clone();
        let records = records.clone();
        let busy = busy.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(&records, "Cardigann import", "API context is unavailable");
                on_error_toast.emit("Cardigann import: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let client = api.client.clone();
            let draft_snapshot = (*draft).clone();
            let definitions = definitions.clone();
            let cardigann_import = cardigann_import.clone();
            let records = records.clone();
            let busy = busy.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                match import_cardigann_definition(&client, &draft_snapshot).await {
                    Ok(response) => {
                        append_json_record(&records, "Cardigann import", &response);
                        cardigann_import.set(CardigannImportState {
                            summary: Some(response.clone()),
                        });
                        match fetch_definitions(&client).await {
                            Ok(definitions_response) => {
                                definitions.set(DefinitionsState {
                                    definitions: definitions_response.definitions,
                                    loaded: true,
                                });
                            }
                            Err(error) => {
                                append_record(&records, "Definitions", error.to_string());
                                on_error_toast.emit(format!("Definitions: {error}"));
                            }
                        }
                        on_success_toast.emit("Cardigann definition imported".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Cardigann import", error.clone());
                        on_error_toast.emit(format!("Cardigann import: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };

    let on_create_tag = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Tag create",
            "Tag created",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { create_tag(&client, &draft_snapshot).await }
            },
        )
    };
    let on_update_tag = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Tag update",
            "Tag updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { update_tag(&client, &draft_snapshot).await }
            },
        )
    };
    let on_delete_tag = {
        let draft_snapshot = (*draft).clone();
        void_action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Tag delete",
            "Tag deleted",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { delete_tag(&client, &draft_snapshot).await }
            },
        )
    };
    let on_fetch_tags = {
        let api = api.clone();
        let busy = busy.clone();
        let records = records.clone();
        let tag_inventory = tag_inventory.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(&records, "Tag inventory", "API context is unavailable");
                on_error_toast.emit("Tag inventory: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let client = api.client.clone();
            let busy = busy.clone();
            let records = records.clone();
            let tag_inventory = tag_inventory.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                match fetch_tags(&client).await {
                    Ok(response) => {
                        append_json_record(&records, "Tag inventory", &response.tags);
                        tag_inventory.set(TagInventoryState {
                            items: response.tags,
                        });
                        on_success_toast.emit("Tag inventory loaded".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Tag inventory", error.clone());
                        on_error_toast.emit(format!("Tag inventory: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_fetch_secret_metadata = {
        let api = api.clone();
        let busy = busy.clone();
        let records = records.clone();
        let secret_inventory = secret_inventory.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(&records, "Secret inventory", "API context is unavailable");
                on_error_toast.emit("Secret inventory: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let client = api.client.clone();
            let busy = busy.clone();
            let records = records.clone();
            let secret_inventory = secret_inventory.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                match fetch_secret_metadata(&client).await {
                    Ok(response) => {
                        append_json_record(&records, "Secret inventory", &response.secrets);
                        secret_inventory.set(SecretInventoryState {
                            items: response.secrets,
                        });
                        on_success_toast.emit("Secret inventory loaded".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Secret inventory", error.clone());
                        on_error_toast.emit(format!("Secret inventory: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_fetch_routing_inventory = {
        let api = api.clone();
        let busy = busy.clone();
        let records = records.clone();
        let routing_inventory = routing_inventory.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(&records, "Routing inventory", "API context is unavailable");
                on_error_toast.emit("Routing inventory: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let client = api.client.clone();
            let busy = busy.clone();
            let records = records.clone();
            let routing_inventory = routing_inventory.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                match fetch_routing_policies(&client).await {
                    Ok(response) => {
                        append_json_record(
                            &records,
                            "Routing inventory",
                            &response.routing_policies,
                        );
                        routing_inventory.set(RoutingPolicyInventoryState {
                            items: response.routing_policies,
                        });
                        on_success_toast.emit("Routing inventory loaded".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Routing inventory", error.clone());
                        on_error_toast.emit(format!("Routing inventory: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_fetch_rate_limit_inventory = {
        let api = api.clone();
        let busy = busy.clone();
        let records = records.clone();
        let rate_limit_inventory = rate_limit_inventory.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(
                    &records,
                    "Rate-limit inventory",
                    "API context is unavailable",
                );
                on_error_toast.emit("Rate-limit inventory: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let client = api.client.clone();
            let busy = busy.clone();
            let records = records.clone();
            let rate_limit_inventory = rate_limit_inventory.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                match fetch_rate_limit_policies(&client).await {
                    Ok(response) => {
                        append_json_record(
                            &records,
                            "Rate-limit inventory",
                            &response.rate_limit_policies,
                        );
                        rate_limit_inventory.set(RateLimitInventoryState {
                            items: response.rate_limit_policies,
                        });
                        on_success_toast.emit("Rate-limit inventory loaded".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Rate-limit inventory", error.clone());
                        on_error_toast.emit(format!("Rate-limit inventory: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_fetch_indexer_instances = {
        let api = api.clone();
        let busy = busy.clone();
        let records = records.clone();
        let indexer_instance_inventory = indexer_instance_inventory.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(
                    &records,
                    "Indexer instance inventory",
                    "API context is unavailable",
                );
                on_error_toast
                    .emit("Indexer instance inventory: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let client = api.client.clone();
            let busy = busy.clone();
            let records = records.clone();
            let indexer_instance_inventory = indexer_instance_inventory.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                match fetch_indexer_instances(&client).await {
                    Ok(response) => {
                        append_json_record(
                            &records,
                            "Indexer instance inventory",
                            &response.indexer_instances,
                        );
                        indexer_instance_inventory.set(IndexerInstanceInventoryState {
                            items: response.indexer_instances,
                        });
                        on_success_toast.emit("Indexer instance inventory loaded".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Indexer instance inventory", error.clone());
                        on_error_toast.emit(format!("Indexer instance inventory: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_fetch_health_notification_hooks = {
        let api = api.clone();
        let busy = busy.clone();
        let records = records.clone();
        let hooks = health_notification_hooks.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(
                    &records,
                    "Health notification hooks",
                    "API context is unavailable",
                );
                on_error_toast
                    .emit("Health notification hooks: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let client = api.client.clone();
            let busy = busy.clone();
            let records = records.clone();
            let hooks = hooks.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                match fetch_health_notification_hooks(&client).await {
                    Ok(response) => {
                        append_json_record(&records, "Health notification hooks", &response.hooks);
                        hooks.set(HealthNotificationHooksState {
                            hooks: response.hooks,
                        });
                        on_success_toast.emit("Health notification hooks loaded".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Health notification hooks", error.clone());
                        on_error_toast.emit(format!("Health notification hooks: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_create_health_notification_hook = {
        let api = api.clone();
        let busy = busy.clone();
        let records = records.clone();
        let hooks = health_notification_hooks.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let draft_snapshot = (*draft).clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(
                    &records,
                    "Health notification hook create",
                    "API context is unavailable",
                );
                on_error_toast.emit(
                    "Health notification hook create: API context is unavailable".to_string(),
                );
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let client = api.client.clone();
            let busy = busy.clone();
            let records = records.clone();
            let hooks = hooks.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            let draft_snapshot = draft_snapshot.clone();
            spawn_local(async move {
                match create_health_notification_hook(&client, &draft_snapshot).await {
                    Ok(response) => {
                        append_json_record(&records, "Health notification hook create", &response);
                        let mut next = (*hooks).clone();
                        next.hooks.push(response);
                        hooks.set(next);
                        on_success_toast.emit("Health notification hook created".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Health notification hook create", error.clone());
                        on_error_toast.emit(format!("Health notification hook create: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_update_health_notification_hook = {
        let api = api.clone();
        let busy = busy.clone();
        let records = records.clone();
        let hooks = health_notification_hooks.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let draft_snapshot = (*draft).clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(
                    &records,
                    "Health notification hook update",
                    "API context is unavailable",
                );
                on_error_toast.emit(
                    "Health notification hook update: API context is unavailable".to_string(),
                );
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let client = api.client.clone();
            let busy = busy.clone();
            let records = records.clone();
            let hooks = hooks.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            let draft_snapshot = draft_snapshot.clone();
            spawn_local(async move {
                match update_health_notification_hook(&client, &draft_snapshot).await {
                    Ok(response) => {
                        append_json_record(&records, "Health notification hook update", &response);
                        let mut next = (*hooks).clone();
                        if let Some(existing) = next.hooks.iter_mut().find(|item| {
                            item.indexer_health_notification_hook_public_id
                                == response.indexer_health_notification_hook_public_id
                        }) {
                            *existing = response;
                        } else {
                            next.hooks.push(response);
                        }
                        hooks.set(next);
                        on_success_toast.emit("Health notification hook updated".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Health notification hook update", error.clone());
                        on_error_toast.emit(format!("Health notification hook update: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_delete_health_notification_hook = {
        let api = api.clone();
        let busy = busy.clone();
        let records = records.clone();
        let hooks = health_notification_hooks.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let draft_snapshot = (*draft).clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(
                    &records,
                    "Health notification hook delete",
                    "API context is unavailable",
                );
                on_error_toast.emit(
                    "Health notification hook delete: API context is unavailable".to_string(),
                );
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let client = api.client.clone();
            let busy = busy.clone();
            let records = records.clone();
            let hooks = hooks.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            let draft_snapshot = draft_snapshot.clone();
            spawn_local(async move {
                match delete_health_notification_hook(&client, &draft_snapshot).await {
                    Ok(()) => {
                        append_record(
                            &records,
                            "Health notification hook delete",
                            "Health notification hook deleted",
                        );
                        let mut next = (*hooks).clone();
                        next.hooks.retain(|item| {
                            item.indexer_health_notification_hook_public_id.to_string()
                                != draft_snapshot.health_notification_hook_public_id
                        });
                        hooks.set(next);
                        on_success_toast.emit("Health notification hook deleted".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Health notification hook delete", error.clone());
                        on_error_toast.emit(format!("Health notification hook delete: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_create_secret = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Secret create",
            "Secret created",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { create_secret(&client, &draft_snapshot).await }
            },
        )
    };
    let on_rotate_secret = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Secret rotate",
            "Secret rotated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { rotate_secret(&client, &draft_snapshot).await }
            },
        )
    };
    let on_revoke_secret = {
        let draft_snapshot = (*draft).clone();
        void_action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Secret revoke",
            "Secret revoked",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { revoke_secret(&client, &draft_snapshot).await }
            },
        )
    };

    let on_create_routing = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Routing policy create",
            "Routing policy created",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { create_routing_policy(&client, &draft_snapshot).await }
            },
        )
    };
    let on_fetch_routing = {
        let api = api.clone();
        let busy = busy.clone();
        let records = records.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let draft_snapshot = (*draft).clone();
        let routing_policy = routing_policy.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(
                    &records,
                    "Routing policy fetch",
                    "API context is unavailable",
                );
                on_error_toast.emit("Routing policy fetch: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let busy = busy.clone();
            let records = records.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            let routing_policy = routing_policy.clone();
            let draft_snapshot = draft_snapshot.clone();
            spawn_local(async move {
                let client = api.client.clone();
                match fetch_routing_policy(&client, &draft_snapshot).await {
                    Ok(value) => {
                        append_json_record(&records, "Routing policy fetch", &value);
                        routing_policy.set(RoutingPolicyState {
                            detail: Some(value),
                        });
                        on_success_toast.emit("Routing policy loaded".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Routing policy fetch", error.clone());
                        on_error_toast.emit(format!("Routing policy fetch: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_set_routing_param = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Routing param set",
            "Routing parameter saved",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { set_routing_param(&client, &draft_snapshot).await }
            },
        )
    };
    let on_bind_routing_secret = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Routing secret bind",
            "Routing secret bound",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { bind_routing_secret(&client, &draft_snapshot).await }
            },
        )
    };
    let on_create_rate_limit = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Rate limit create",
            "Rate limit policy created",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { create_rate_limit_policy(&client, &draft_snapshot).await }
            },
        )
    };
    let on_update_rate_limit = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Rate limit update",
            "Rate limit policy updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { update_rate_limit_policy(&client, &draft_snapshot).await }
            },
        )
    };
    let on_delete_rate_limit = {
        let draft_snapshot = (*draft).clone();
        void_action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Rate limit delete",
            "Rate limit policy deleted",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { delete_rate_limit_policy(&client, &draft_snapshot).await }
            },
        )
    };
    let on_assign_rate_limit_indexer = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Rate limit assign indexer",
            "Rate limit assigned to indexer",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { assign_rate_limit_to_indexer(&client, &draft_snapshot).await }
            },
        )
    };
    let on_assign_rate_limit_routing = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Rate limit assign routing",
            "Rate limit assigned to routing policy",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { assign_rate_limit_to_routing(&client, &draft_snapshot).await }
            },
        )
    };
    let on_upsert_tracker_category_mapping = {
        let draft_snapshot = (*draft).clone();
        void_action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Tracker category mapping upsert",
            "Tracker category mapping updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { upsert_tracker_category_mapping(&client, &draft_snapshot).await }
            },
        )
    };
    let on_delete_tracker_category_mapping = {
        let draft_snapshot = (*draft).clone();
        void_action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Tracker category mapping delete",
            "Tracker category mapping deleted",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { delete_tracker_category_mapping(&client, &draft_snapshot).await }
            },
        )
    };

    let on_create_instance = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Indexer instance create",
            "Indexer instance created",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { create_indexer_instance(&client, &draft_snapshot).await }
            },
        )
    };
    let on_update_instance = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Indexer instance update",
            "Indexer instance updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { update_indexer_instance(&client, &draft_snapshot).await }
            },
        )
    };
    let on_fetch_rss_subscription = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "RSS subscription fetch",
            "RSS subscription loaded",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { fetch_indexer_rss_subscription(&client, &draft_snapshot).await }
            },
        )
    };
    let on_update_rss_subscription = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "RSS subscription update",
            "RSS subscription updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { update_indexer_rss_subscription(&client, &draft_snapshot).await }
            },
        )
    };
    let on_fetch_rss_items = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "RSS items fetch",
            "RSS items loaded",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { fetch_indexer_rss_items(&client, &draft_snapshot).await }
            },
        )
    };
    let on_mark_rss_item_seen = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "RSS item mark seen",
            "RSS item recorded as seen",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { mark_indexer_rss_item_seen(&client, &draft_snapshot).await }
            },
        )
    };
    let on_fetch_connectivity_profile = {
        let draft_snapshot = (*draft).clone();
        let connectivity = connectivity.clone();
        let records = records.clone();
        let busy = busy.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let api = api.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(
                    &records,
                    "Connectivity profile fetch",
                    "API context is unavailable",
                );
                on_error_toast.emit("Indexer API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let client = api.client.clone();
            let draft_snapshot = draft_snapshot.clone();
            let connectivity = connectivity.clone();
            let records = records.clone();
            let busy = busy.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                match fetch_indexer_connectivity_profile(&client, &draft_snapshot).await {
                    Ok(profile) => {
                        append_json_record(&records, "Connectivity profile fetch", &profile);
                        let mut next = (*connectivity).clone();
                        next.profile = Some(profile);
                        connectivity.set(next);
                        on_success_toast.emit("Connectivity profile loaded".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Connectivity profile fetch", error.clone());
                        on_error_toast.emit(format!("Connectivity profile fetch: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_fetch_source_reputation = {
        let draft_snapshot = (*draft).clone();
        let connectivity = connectivity.clone();
        let records = records.clone();
        let busy = busy.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let api = api.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(
                    &records,
                    "Source reputation fetch",
                    "API context is unavailable",
                );
                on_error_toast.emit("Indexer API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let client = api.client.clone();
            let draft_snapshot = draft_snapshot.clone();
            let connectivity = connectivity.clone();
            let records = records.clone();
            let busy = busy.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                match fetch_indexer_source_reputation(&client, &draft_snapshot).await {
                    Ok(response) => {
                        let items = response.items;
                        append_json_record(&records, "Source reputation fetch", &items);
                        let mut next = (*connectivity).clone();
                        next.reputation_items = items;
                        connectivity.set(next);
                        on_success_toast.emit("Source reputation loaded".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Source reputation fetch", error.clone());
                        on_error_toast.emit(format!("Source reputation fetch: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_fetch_health_events = {
        let draft_snapshot = (*draft).clone();
        let health_events = health_events.clone();
        let records = records.clone();
        let busy = busy.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let api = api.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(
                    &records,
                    "Health events fetch",
                    "API context is unavailable",
                );
                on_error_toast.emit("Indexer API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let client = api.client.clone();
            let draft_snapshot = draft_snapshot.clone();
            let health_events = health_events.clone();
            let records = records.clone();
            let busy = busy.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                match fetch_indexer_health_events(&client, &draft_snapshot).await {
                    Ok(response) => {
                        let items = response.items;
                        append_json_record(&records, "Health events fetch", &items);
                        health_events.set(HealthEventsState { items });
                        on_success_toast.emit("Health events loaded".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Health events fetch", error.clone());
                        on_error_toast.emit(format!("Health events fetch: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_set_media_domains = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Indexer media domains",
            "Indexer media domains updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { set_indexer_media_domains(&client, &draft_snapshot).await }
            },
        )
    };
    let on_set_tags = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Indexer tags",
            "Indexer tags updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { set_indexer_tags(&client, &draft_snapshot).await }
            },
        )
    };
    let on_set_field_value = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Indexer field value",
            "Indexer field value updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { set_indexer_field_value(&client, &draft_snapshot).await }
            },
        )
    };
    let on_bind_field_secret = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Indexer field secret",
            "Indexer field secret bound",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { bind_indexer_field_secret(&client, &draft_snapshot).await }
            },
        )
    };
    let on_fetch_cf_state = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "CF state fetch",
            "Cloudflare state loaded",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { fetch_cf_state(&client, &draft_snapshot).await }
            },
        )
    };
    let on_reset_cf_state = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "CF state reset",
            "Cloudflare state reset",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { reset_cf_state(&client, &draft_snapshot).await }
            },
        )
    };
    let on_prepare_test = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Indexer test prepare",
            "Indexer test prepared",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { prepare_indexer_test(&client, &draft_snapshot).await }
            },
        )
    };
    let on_finalize_test = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Indexer test finalize",
            "Indexer test finalized",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { finalize_indexer_test(&client, &draft_snapshot).await }
            },
        )
    };

    let on_create_profile = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Search profile create",
            "Search profile created",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { create_search_profile(&client, &draft_snapshot).await }
            },
        )
    };
    let on_update_profile = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Search profile update",
            "Search profile updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { update_search_profile(&client, &draft_snapshot).await }
            },
        )
    };
    let on_set_profile_default_domain = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Search profile default domain",
            "Search profile default domain updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { set_search_profile_default_domain(&client, &draft_snapshot).await }
            },
        )
    };
    let on_set_profile_media_domains = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Search profile media domains",
            "Search profile media domains updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { set_search_profile_media_domains(&client, &draft_snapshot).await }
            },
        )
    };
    let on_add_policy_set = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Search profile policy set",
            "Policy set attached to search profile",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { add_search_profile_policy_set(&client, &draft_snapshot).await }
            },
        )
    };
    let on_allow_indexers = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Search profile indexers allow",
            "Search profile allow-list updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { set_search_profile_indexers(&client, &draft_snapshot, "allow").await }
            },
        )
    };
    let on_block_indexers = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Search profile indexers block",
            "Search profile block-list updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { set_search_profile_indexers(&client, &draft_snapshot, "block").await }
            },
        )
    };
    let on_allow_tags = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Search profile tags allow",
            "Search profile allow tags updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move {
                    let tag_value = draft_snapshot.search_profile_tag_keys_allow.clone();
                    set_search_profile_tags(&client, &draft_snapshot, "allow", &tag_value).await
                }
            },
        )
    };
    let on_block_tags = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Search profile tags block",
            "Search profile block tags updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move {
                    let tag_value = draft_snapshot.search_profile_tag_keys_block.clone();
                    set_search_profile_tags(&client, &draft_snapshot, "block", &tag_value).await
                }
            },
        )
    };
    let on_prefer_tags = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Search profile tags prefer",
            "Search profile preferred tags updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move {
                    let tag_value = draft_snapshot.search_profile_tag_keys_prefer.clone();
                    set_search_profile_tags(&client, &draft_snapshot, "prefer", &tag_value).await
                }
            },
        )
    };
    let on_create_policy_set = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Policy set create",
            "Policy set created",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { create_policy_set(&client, &draft_snapshot).await }
            },
        )
    };
    let on_create_policy_rule = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Policy rule create",
            "Policy rule created",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { create_policy_rule(&client, &draft_snapshot).await }
            },
        )
    };

    let on_create_import_job = {
        let api = api.clone();
        let busy = busy.clone();
        let draft = draft.clone();
        let import_job = import_job.clone();
        let records = records.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let draft_snapshot = (*draft).clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(&records, "Import job create", "API context is unavailable");
                on_error_toast.emit("Import job create: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let busy = busy.clone();
            let draft = draft.clone();
            let import_job = import_job.clone();
            let records = records.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            let draft_snapshot = draft_snapshot.clone();
            spawn_local(async move {
                let client = api.client.clone();
                match create_import_job(&client, &draft_snapshot).await {
                    Ok(value) => {
                        append_json_record(&records, "Import job create", &value);
                        let mut next = (*draft).clone();
                        next.import_job_public_id = value.import_job_public_id.to_string();
                        draft.set(next);
                        import_job.set(ImportJobState::default());
                        on_success_toast.emit("Import job created".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Import job create", error.clone());
                        on_error_toast.emit(format!("Import job create: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_run_import_api = {
        let api = api.clone();
        let busy = busy.clone();
        let draft = draft.clone();
        let records = records.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let draft_snapshot = (*draft).clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(
                    &records,
                    "Import job run (Prowlarr API)",
                    "API context is unavailable",
                );
                on_error_toast
                    .emit("Import job run (Prowlarr API): API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let busy = busy.clone();
            let draft = draft.clone();
            let records = records.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            let draft_snapshot = draft_snapshot.clone();
            spawn_local(async move {
                let client = api.client.clone();
                match run_import_job_api(&client, &draft_snapshot).await {
                    Ok(value) => {
                        append_json_record(&records, "Import job run (Prowlarr API)", &value);
                        let mut next = (*draft).clone();
                        next.import_job_public_id = value.import_job_public_id.to_string();
                        draft.set(next);
                        on_success_toast.emit("Prowlarr API import started".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Import job run (Prowlarr API)", error.clone());
                        on_error_toast.emit(format!("Import job run (Prowlarr API): {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_run_import_backup = {
        let api = api.clone();
        let busy = busy.clone();
        let draft = draft.clone();
        let records = records.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let draft_snapshot = (*draft).clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(
                    &records,
                    "Import job run (backup)",
                    "API context is unavailable",
                );
                on_error_toast
                    .emit("Import job run (backup): API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let busy = busy.clone();
            let draft = draft.clone();
            let records = records.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            let draft_snapshot = draft_snapshot.clone();
            spawn_local(async move {
                let client = api.client.clone();
                match run_import_job_backup(&client, &draft_snapshot).await {
                    Ok(value) => {
                        append_json_record(&records, "Import job run (backup)", &value);
                        let mut next = (*draft).clone();
                        next.import_job_public_id = value.import_job_public_id.to_string();
                        draft.set(next);
                        on_success_toast.emit("Prowlarr backup import started".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Import job run (backup)", error.clone());
                        on_error_toast.emit(format!("Import job run (backup): {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_fetch_import_status = {
        let api = api.clone();
        let busy = busy.clone();
        let import_job = import_job.clone();
        let records = records.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let draft_snapshot = (*draft).clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(&records, "Import job status", "API context is unavailable");
                on_error_toast.emit("Import job status: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let busy = busy.clone();
            let import_job = import_job.clone();
            let records = records.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            let draft_snapshot = draft_snapshot.clone();
            spawn_local(async move {
                let client = api.client.clone();
                match fetch_import_job_status(&client, &draft_snapshot).await {
                    Ok(value) => {
                        append_json_record(&records, "Import job status", &value);
                        import_job.set(ImportJobState {
                            status: Some(value),
                            results: (*import_job).results.clone(),
                        });
                        on_success_toast.emit("Import job status loaded".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Import job status", error.clone());
                        on_error_toast.emit(format!("Import job status: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_fetch_import_results = {
        let api = api.clone();
        let busy = busy.clone();
        let import_job = import_job.clone();
        let records = records.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let draft_snapshot = (*draft).clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(&records, "Import job results", "API context is unavailable");
                on_error_toast.emit("Import job results: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let busy = busy.clone();
            let import_job = import_job.clone();
            let records = records.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            let draft_snapshot = draft_snapshot.clone();
            spawn_local(async move {
                let client = api.client.clone();
                match fetch_import_job_results(&client, &draft_snapshot).await {
                    Ok(value) => {
                        append_json_record(&records, "Import job results", &value);
                        import_job.set(ImportJobState {
                            status: (*import_job).status.clone(),
                            results: value.results,
                        });
                        on_success_toast.emit("Import job results loaded".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Import job results", error.clone());
                        on_error_toast.emit(format!("Import job results: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_fetch_source_conflicts = {
        let api = api.clone();
        let busy = busy.clone();
        let source_conflicts = source_conflicts.clone();
        let records = records.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let draft_snapshot = (*draft).clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(&records, "Source conflicts", "API context is unavailable");
                on_error_toast.emit("Source conflicts: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let busy = busy.clone();
            let source_conflicts = source_conflicts.clone();
            let records = records.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            let draft_snapshot = draft_snapshot.clone();
            spawn_local(async move {
                let client = api.client.clone();
                match fetch_source_metadata_conflicts(&client, &draft_snapshot).await {
                    Ok(value) => {
                        append_json_record(&records, "Source conflicts", &value);
                        source_conflicts.set(SourceMetadataConflictsState {
                            items: value.conflicts,
                        });
                        on_success_toast.emit("Source conflicts loaded".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Source conflicts", error.clone());
                        on_error_toast.emit(format!("Source conflicts: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_resolve_source_conflict = {
        let api = api.clone();
        let busy = busy.clone();
        let records = records.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let draft_snapshot = (*draft).clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(
                    &records,
                    "Source conflict resolve",
                    "API context is unavailable",
                );
                on_error_toast
                    .emit("Source conflict resolve: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let busy = busy.clone();
            let records = records.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            let draft_snapshot = draft_snapshot.clone();
            spawn_local(async move {
                let client = api.client.clone();
                match resolve_source_metadata_conflict(&client, &draft_snapshot).await {
                    Ok(()) => {
                        append_record(
                            &records,
                            "Source conflict resolve",
                            "Conflict resolution submitted",
                        );
                        on_success_toast.emit("Source conflict resolved".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Source conflict resolve", error.clone());
                        on_error_toast.emit(format!("Source conflict resolve: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_reopen_source_conflict = {
        let api = api.clone();
        let busy = busy.clone();
        let records = records.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        let draft_snapshot = (*draft).clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(
                    &records,
                    "Source conflict reopen",
                    "API context is unavailable",
                );
                on_error_toast
                    .emit("Source conflict reopen: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let busy = busy.clone();
            let records = records.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            let draft_snapshot = draft_snapshot.clone();
            spawn_local(async move {
                let client = api.client.clone();
                match reopen_source_metadata_conflict(&client, &draft_snapshot).await {
                    Ok(()) => {
                        append_record(&records, "Source conflict reopen", "Conflict reopened");
                        on_success_toast.emit("Source conflict reopened".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Source conflict reopen", error.clone());
                        on_error_toast.emit(format!("Source conflict reopen: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_export_backup = {
        let api = api.clone();
        let busy = busy.clone();
        let backup = backup.clone();
        let draft = draft.clone();
        let records = records.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(&records, "Backup export", "API context is unavailable");
                on_error_toast.emit("Backup export: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let busy = busy.clone();
            let backup = backup.clone();
            let draft = draft.clone();
            let records = records.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                let client = api.client.clone();
                match export_backup_snapshot(&client).await {
                    Ok(snapshot) => {
                        append_json_record(&records, "Backup export", &snapshot);
                        let mut next_draft = (*draft).clone();
                        next_draft.backup_snapshot_payload =
                            serde_json::to_string_pretty(&snapshot).unwrap_or_default();
                        draft.set(next_draft);
                        backup.set(BackupState {
                            snapshot: Some(snapshot),
                            unresolved_secret_bindings: Vec::new(),
                        });
                        on_success_toast.emit("Indexer backup exported".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Backup export", error.clone());
                        on_error_toast.emit(format!("Backup export: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_restore_backup = {
        let api = api.clone();
        let busy = busy.clone();
        let backup = backup.clone();
        let draft = draft.clone();
        let records = records.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(&records, "Backup restore", "API context is unavailable");
                on_error_toast.emit("Backup restore: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            let payload = (*draft).backup_snapshot_payload.trim().to_string();
            let snapshot: IndexerBackupSnapshot = match serde_json::from_str(&payload) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    let message = format!("Invalid backup snapshot JSON: {error}");
                    append_record(&records, "Backup restore", message.clone());
                    on_error_toast.emit(format!("Backup restore: {message}"));
                    return;
                }
            };
            busy.set(true);
            let busy = busy.clone();
            let backup = backup.clone();
            let records = records.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                let client = api.client.clone();
                match restore_backup_snapshot(&client, &snapshot).await {
                    Ok(response) => {
                        append_json_record(&records, "Backup restore", &response);
                        backup.set(BackupState {
                            snapshot: Some(snapshot),
                            unresolved_secret_bindings: response.unresolved_secret_bindings,
                        });
                        on_success_toast.emit("Indexer backup restored".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "Backup restore", error.clone());
                        on_error_toast.emit(format!("Backup restore: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_create_torznab = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Torznab instance create",
            "Torznab instance created",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { create_torznab_instance(&client, &draft_snapshot).await }
            },
        )
    };
    let on_rotate_torznab = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Torznab key rotate",
            "Torznab API key rotated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { rotate_torznab_key(&client, &draft_snapshot).await }
            },
        )
    };
    let on_provision_app_sync = {
        let api = api.clone();
        let draft = draft.clone();
        let app_sync = app_sync.clone();
        let busy = busy.clone();
        let records = records.clone();
        let on_success_toast = props.on_success_toast.clone();
        let on_error_toast = props.on_error_toast.clone();
        Callback::from(move |_| {
            let Some(api) = api.clone() else {
                append_record(&records, "App sync", "API context is unavailable");
                on_error_toast.emit("App sync: API context is unavailable".to_string());
                return;
            };
            if *busy {
                return;
            }
            busy.set(true);
            let draft = draft.clone();
            let app_sync = app_sync.clone();
            let busy = busy.clone();
            let records = records.clone();
            let on_success_toast = on_success_toast.clone();
            let on_error_toast = on_error_toast.clone();
            spawn_local(async move {
                let client = api.client.clone();
                let draft_snapshot = (*draft).clone();
                match provision_app_sync(&client, &draft_snapshot).await {
                    Ok(summary) => {
                        append_json_record(&records, "App sync", &summary);
                        let mut next = (*draft).clone();
                        next.search_profile_public_id =
                            summary.search_profile_public_id.to_string();
                        next.torznab_search_profile_public_id =
                            summary.search_profile_public_id.to_string();
                        next.torznab_instance_public_id =
                            summary.torznab_instance_public_id.to_string();
                        draft.set(next);
                        app_sync.set(AppSyncState {
                            summary: Some(summary),
                        });
                        on_success_toast.emit("App sync provisioned".to_string());
                    }
                    Err(error) => {
                        append_record(&records, "App sync", error.clone());
                        on_error_toast.emit(format!("App sync: {error}"));
                    }
                }
                busy.set(false);
            });
        })
    };
    let on_set_torznab_state = {
        let draft_snapshot = (*draft).clone();
        action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Torznab state update",
            "Torznab state updated",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { set_torznab_state(&client, &draft_snapshot).await }
            },
        )
    };
    let on_delete_torznab = {
        let draft_snapshot = (*draft).clone();
        void_action_callback(
            api.clone(),
            busy.clone(),
            records.clone(),
            props.on_success_toast.clone(),
            props.on_error_toast.clone(),
            "Torznab delete",
            "Torznab instance deleted",
            move |api| {
                let client = api.client.clone();
                let draft_snapshot = draft_snapshot.clone();
                async move { delete_torznab_instance(&client, &draft_snapshot).await }
            },
        )
    };

    html! {
        <section class="space-y-4" data-testid="indexers-page">
            <div class="rounded-box border border-base-300 bg-base-100 p-6 shadow-sm">
                <h1 class="text-2xl font-semibold">{"Indexers"}</h1>
                <p class="mt-2 max-w-3xl text-sm text-base-content/70">
                    {"This admin console exposes the ERD-backed indexer operations currently available in the API: catalog lookup, tags, health notification hooks, secrets, routing, rate limits, instances, search profiles, policies, imports, and Torznab management."}
                </p>
            </div>
            <DefinitionsSection
                draft={draft.clone()}
                definitions={(*definitions).clone()}
                cardigann_import={(*cardigann_import).clone()}
                busy={*definitions_busy || *busy}
                on_refresh={on_refresh_definitions}
                on_import_cardigann={on_import_cardigann}
            />
            <TagSecretSection
                draft={draft.clone()}
                tags={(*tag_inventory).clone()}
                busy={*busy}
                on_fetch_tags={on_fetch_tags}
                on_primary={on_create_tag}
                on_secondary={on_update_tag}
                on_tertiary={on_delete_tag}
            />
            <HealthNotificationsSection
                draft={draft.clone()}
                hooks={(*health_notification_hooks).clone()}
                busy={*busy}
                on_fetch_hooks={on_fetch_health_notification_hooks}
                on_create_hook={on_create_health_notification_hook}
                on_update_hook={on_update_health_notification_hook}
                on_delete_hook={on_delete_health_notification_hook}
            />
            <SecretSection
                draft={draft.clone()}
                secrets={(*secret_inventory).clone()}
                busy={*busy}
                on_fetch_secrets={on_fetch_secret_metadata}
                on_primary={on_create_secret}
                on_secondary={on_rotate_secret}
                on_tertiary={on_revoke_secret}
            />
            <RoutingRateLimitSection
                draft={draft.clone()}
                routing_policy={(*routing_policy).clone()}
                routing_inventory={(*routing_inventory).clone()}
                rate_limit_inventory={(*rate_limit_inventory).clone()}
                busy={*busy}
                on_fetch_routing_inventory={on_fetch_routing_inventory}
                on_fetch_routing={on_fetch_routing}
                on_create_routing={on_create_routing}
                on_set_routing_param={on_set_routing_param}
                on_bind_routing_secret={on_bind_routing_secret}
                on_fetch_rate_limits={on_fetch_rate_limit_inventory}
                on_create_rate_limit={on_create_rate_limit}
                on_update_rate_limit={on_update_rate_limit}
                on_delete_rate_limit={on_delete_rate_limit}
                on_assign_indexer={on_assign_rate_limit_indexer}
                on_assign_routing={on_assign_rate_limit_routing}
            />
            <InstancesSection
                draft={draft.clone()}
                instances={(*indexer_instance_inventory).clone()}
                connectivity={(*connectivity).clone()}
                health_events={(*health_events).clone()}
                busy={*busy}
                on_fetch_instances={on_fetch_indexer_instances}
                on_create_instance={on_create_instance}
                on_update_instance={on_update_instance}
                on_fetch_rss_subscription={on_fetch_rss_subscription}
                on_update_rss_subscription={on_update_rss_subscription}
                on_fetch_rss_items={on_fetch_rss_items}
                on_mark_rss_item_seen={on_mark_rss_item_seen}
                on_fetch_connectivity_profile={on_fetch_connectivity_profile}
                on_fetch_health_events={on_fetch_health_events}
                on_fetch_source_reputation={on_fetch_source_reputation}
                on_set_media_domains={on_set_media_domains}
                on_set_tags={on_set_tags}
                on_upsert_tracker_category_mapping={on_upsert_tracker_category_mapping}
                on_delete_tracker_category_mapping={on_delete_tracker_category_mapping}
                on_set_field_value={on_set_field_value}
                on_bind_field_secret={on_bind_field_secret}
                on_fetch_cf_state={on_fetch_cf_state}
                on_reset_cf_state={on_reset_cf_state}
                on_prepare_test={on_prepare_test}
                on_finalize_test={on_finalize_test}
            />
            <ProfilesPoliciesSection
                draft={draft.clone()}
                busy={*busy}
                on_create_profile={on_create_profile}
                on_update_profile={on_update_profile}
                on_default_domain={on_set_profile_default_domain}
                on_media_domains={on_set_profile_media_domains}
                on_add_policy_set={on_add_policy_set}
                on_allow_indexers={on_allow_indexers}
                on_block_indexers={on_block_indexers}
                on_allow_tags={on_allow_tags}
                on_block_tags={on_block_tags}
                on_prefer_tags={on_prefer_tags}
                on_create_policy_set={on_create_policy_set}
                on_create_policy_rule={on_create_policy_rule}
            />
                <ImportTorznabSection
                    draft={draft.clone()}
                    app_sync={(*app_sync).clone()}
                    import_job={(*import_job).clone()}
                    source_conflicts={(*source_conflicts).clone()}
                    backup={(*backup).clone()}
                    busy={*busy}
                    on_provision_app_sync={on_provision_app_sync}
                    on_create_import_job={on_create_import_job}
                    on_run_import_api={on_run_import_api}
                    on_run_import_backup={on_run_import_backup}
                    on_fetch_import_status={on_fetch_import_status}
                    on_fetch_import_results={on_fetch_import_results}
                    on_fetch_source_conflicts={on_fetch_source_conflicts}
                    on_resolve_source_conflict={on_resolve_source_conflict}
                    on_reopen_source_conflict={on_reopen_source_conflict}
                    on_export_backup={on_export_backup}
                    on_restore_backup={on_restore_backup}
                on_create_torznab={on_create_torznab}
                on_rotate_torznab={on_rotate_torznab}
                on_set_torznab_state={on_set_torznab_state}
                on_delete_torznab={on_delete_torznab}
            />
            <ActivityLog records={(*records).clone()} />
        </section>
    }
}

#[function_component(DefinitionsSection)]
fn definitions_section(props: &DefinitionsSectionProps) -> Html {
    let on_filter = text_callback(props.draft.clone(), |draft, value| {
        draft.definitions_filter = value;
    });
    let on_cardigann_yaml = text_callback(props.draft.clone(), |draft, value| {
        draft.cardigann_yaml_payload = value;
    });
    let on_cardigann_deprecated = bool_callback(props.draft.clone(), |draft, value| {
        draft.cardigann_is_deprecated = value;
    });
    let filtered = filtered_definitions(
        &props.definitions.definitions,
        &props.draft.definitions_filter,
    );
    card(
        "Catalog",
        html! {
            <div class="space-y-4">
                <div class="flex flex-col gap-3 md:flex-row md:items-end">
                    <div class="grow">
                        {field(
                            "Definition filter",
                            "Filter by slug, display name, engine, or protocol.",
                            html! {
                                <Input
                                    value={props.draft.definitions_filter.clone()}
                                    placeholder={Some(AttrValue::from("Search catalog definitions"))}
                                    oninput={on_filter}
                                    disabled={props.busy}
                                    class={classes!("w-full")}
                                />
                            },
                        )}
                    </div>
                    <Button onclick={props.on_refresh.clone()} disabled={props.busy}>
                        {"Refresh definitions"}
                    </Button>
                </div>
                <div class="space-y-3 rounded-box border border-base-300 p-4">
                    <h3 class="font-semibold">{"Cardigann import"}</h3>
                    <p class="text-sm text-base-content/70">
                        {"Paste a Cardigann YAML definition to normalize it into the catalog with typed fields and select options. This closes the remaining manual catalog import gap beyond the existing Prowlarr job flow."}
                    </p>
                    {field("Cardigann YAML", "Raw Cardigann definition document.", html! {
                        <Textarea
                            value={props.draft.cardigann_yaml_payload.clone()}
                            rows={12}
                            oninput={on_cardigann_yaml}
                            disabled={props.busy}
                            class={classes!("w-full", "font-mono", "text-xs")}
                        />
                    })}
                    <label class="flex items-center gap-3">
                        <input
                            type="checkbox"
                            class="checkbox"
                            checked={props.draft.cardigann_is_deprecated}
                            onchange={on_cardigann_deprecated}
                            disabled={props.busy}
                        />
                        <span>{"Mark imported definition as deprecated"}</span>
                    </label>
                    <div class="flex flex-wrap gap-2">
                        <Button onclick={props.on_import_cardigann.clone()} disabled={props.busy}>
                            {"Import Cardigann YAML"}
                        </Button>
                    </div>
                    {
                        if let Some(summary) = props.cardigann_import.summary.as_ref() {
                            render_cardigann_import_summary(summary)
                        } else {
                            html! {
                                <p class="text-sm text-base-content/60">
                                    {"Successful imports will show the imported slug, hash, and field counts here."}
                                </p>
                            }
                        }
                    }
                </div>
                {if filtered.is_empty() {
                    html! { <EmptyState title="No matching definitions" body="Try a broader filter or refresh the catalog." /> }
                } else {
                    html! {
                        <div class="grid gap-3 lg:grid-cols-2">
                            {for filtered.into_iter().map(render_definition_card)}
                        </div>
                    }
                }}
            </div>
        },
    )
}

fn render_definition_card(definition: &IndexerDefinitionResponse) -> Html {
    html! {
        <article class="rounded-box border border-base-300 bg-base-200/40 p-4">
            <div class="flex items-start justify-between gap-3">
                <div>
                    <h3 class="font-semibold">{definition.display_name.clone()}</h3>
                    <p class="text-xs text-base-content/60">{definition.upstream_slug.clone()}</p>
                </div>
                <span class="badge badge-outline">{definition.engine.clone()}</span>
            </div>
            <div class="mt-3 flex flex-wrap gap-2 text-xs">
                <span class="badge badge-ghost">{definition.upstream_source.clone()}</span>
                <span class="badge badge-ghost">{definition.protocol.clone()}</span>
                <span class="badge badge-ghost">{format!("schema: {}", definition.schema_version)}</span>
                <span class="badge badge-ghost">{format!("deprecated: {}", definition.is_deprecated)}</span>
            </div>
        </article>
    }
}

fn render_cardigann_import_summary(summary: &CardigannDefinitionImportResponse) -> Html {
    html! {
        <div class="rounded-box border border-base-300 bg-base-200/40 p-4">
            <div class="flex flex-wrap items-start justify-between gap-3">
                <div>
                    <h4 class="font-semibold">{summary.definition.display_name.clone()}</h4>
                    <p class="text-xs text-base-content/60">{summary.definition.upstream_slug.clone()}</p>
                </div>
                <span class="badge badge-outline">{summary.definition.definition_hash.clone()}</span>
            </div>
            <div class="mt-3 flex flex-wrap gap-2 text-xs">
                <span class="badge badge-ghost">{summary.definition.upstream_source.clone()}</span>
                <span class="badge badge-ghost">{summary.definition.engine.clone()}</span>
                <span class="badge badge-ghost">{format!("fields: {}", summary.field_count)}</span>
                <span class="badge badge-ghost">{format!("options: {}", summary.option_count)}</span>
                <span class="badge badge-ghost">{format!("deprecated: {}", summary.definition.is_deprecated)}</span>
            </div>
        </div>
    }
}

fn render_tag_inventory_item(
    item: &TagListItemResponse,
    on_use_tag: Callback<MouseEvent>,
    on_append_indexer_tag: Callback<MouseEvent>,
    on_append_allow_tag: Callback<MouseEvent>,
) -> Html {
    html! {
        <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
            <div class="flex flex-wrap items-start justify-between gap-2">
                <div>
                    <div class="font-semibold">{item.display_name.clone()}</div>
                    <div class="text-sm text-base-content/70">{item.tag_key.clone()}</div>
                </div>
                <div class="text-xs font-mono break-all text-base-content/60">
                    {item.tag_public_id.to_string()}
                </div>
            </div>
            <div class="mt-3 flex flex-wrap gap-2">
                <Button onclick={on_use_tag}>{"Use for CRUD"}</Button>
                <Button onclick={on_append_indexer_tag}>{"Add to indexer tags"}</Button>
                <Button onclick={on_append_allow_tag}>{"Add to allow tags"}</Button>
            </div>
        </div>
    }
}

fn render_routing_inventory_item(
    item: &RoutingPolicyListItemResponse,
    on_use_policy: Callback<MouseEvent>,
    on_assign_rate_limit: Callback<MouseEvent>,
) -> Html {
    html! {
        <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
            <div class="flex flex-wrap items-start justify-between gap-2">
                <div>
                    <div class="font-semibold">{item.display_name.clone()}</div>
                    <div class="text-sm text-base-content/70">
                        {format!(
                            "{} | params: {} | secret binds: {}",
                            item.mode, item.parameter_count, item.secret_binding_count
                        )}
                    </div>
                </div>
                <div class="text-xs font-mono break-all text-base-content/60">
                    {item.routing_policy_public_id.to_string()}
                </div>
            </div>
            {
                item.rate_limit_display_name.as_ref().map(|name| html! {
                    <div class="mt-2 text-sm text-base-content/70">
                        {format!("Rate limit: {name}")}
                    </div>
                }).unwrap_or_default()
            }
            <div class="mt-3 flex flex-wrap gap-2">
                <Button onclick={on_use_policy}>{"Use for params/binds"}</Button>
                <Button onclick={on_assign_rate_limit}>{"Use for rate-limit assign"}</Button>
            </div>
        </div>
    }
}

fn render_rate_limit_inventory_item(
    item: &RateLimitPolicyListItemResponse,
    on_use_policy: Callback<MouseEvent>,
    on_assign_indexer: Callback<MouseEvent>,
    on_assign_routing: Callback<MouseEvent>,
) -> Html {
    let system_badge = if item.is_system {
        html! { <span class="badge badge-outline">{"system"}</span> }
    } else {
        Html::default()
    };

    html! {
        <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
            <div class="flex flex-wrap items-start justify-between gap-2">
                <div>
                    <div class="font-semibold">{item.display_name.clone()}</div>
                    <div class="text-sm text-base-content/70">
                        {format!(
                            "rpm {} | burst {} | concurrent {}",
                            item.requests_per_minute, item.burst, item.concurrent_requests
                        )}
                    </div>
                </div>
                {system_badge}
            </div>
            <div class="mt-2 text-xs font-mono break-all text-base-content/60">
                {item.rate_limit_policy_public_id.to_string()}
            </div>
            <div class="mt-3 flex flex-wrap gap-2">
                <Button onclick={on_use_policy}>{"Use for update/delete"}</Button>
                <Button onclick={on_assign_indexer}>{"Assign to indexer"}</Button>
                <Button onclick={on_assign_routing}>{"Assign to routing"}</Button>
            </div>
        </div>
    }
}

fn render_indexer_instance_inventory_item(
    item: &IndexerInstanceListItemResponse,
    on_use_instance: Callback<MouseEvent>,
    on_use_rss: Callback<MouseEvent>,
    on_use_assignment: Callback<MouseEvent>,
) -> Html {
    html! {
        <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
            <div class="flex flex-wrap items-start justify-between gap-2">
                <div>
                    <div class="font-semibold">{item.display_name.clone()}</div>
                    <div class="text-sm text-base-content/70">
                        {format!(
                            "{} | rss {} | auto {} | interactive {}",
                            item.upstream_slug,
                            item.rss_status,
                            item.automatic_search_status,
                            item.interactive_search_status
                        )}
                    </div>
                </div>
                <div class="text-xs font-mono break-all text-base-content/60">
                    {item.indexer_instance_public_id.to_string()}
                </div>
            </div>
            <div class="mt-2 grid gap-2 text-sm text-base-content/70 md:grid-cols-2">
                <div>{format!("status: {} | priority {}", item.instance_status, item.priority)}</div>
                <div>{format!("trust: {}", item.trust_tier_key.clone().unwrap_or_else(|| "none".to_string()))}</div>
                <div>{format!("domains: {}", item.media_domain_keys.join(", "))}</div>
                <div>{format!("tags: {}", item.tag_keys.join(", "))}</div>
            </div>
            <div class="mt-3 flex flex-wrap gap-2">
                <Button onclick={on_use_instance}>{"Use for update/fields"}</Button>
                <Button onclick={on_use_rss}>{"Use for RSS/test"}</Button>
                <Button onclick={on_use_assignment}>{"Use for rate-limit assign"}</Button>
            </div>
        </div>
    }
}

fn render_secret_inventory_item(
    item: &SecretMetadataResponse,
    on_use_secret: Callback<MouseEvent>,
    on_use_routing_secret: Callback<MouseEvent>,
    on_use_field_secret: Callback<MouseEvent>,
    on_use_prowlarr_secret: Callback<MouseEvent>,
) -> Html {
    let rotated_at = item
        .rotated_at
        .map_or_else(|| "never".to_string(), |value| value.to_rfc3339());
    let state_badge = if item.is_revoked {
        "badge badge-error badge-outline"
    } else {
        "badge badge-success badge-outline"
    };

    html! {
        <div class="rounded-box border border-base-300 bg-base-200/40 p-3">
            <div class="flex flex-wrap items-start justify-between gap-2">
                <div>
                    <div class="font-semibold">{item.secret_type.clone()}</div>
                    <div class="text-sm text-base-content/70">
                        {format!("bindings: {} | rotated: {}", item.binding_count, rotated_at)}
                    </div>
                </div>
                <span class={state_badge}>{if item.is_revoked { "revoked" } else { "active" }}</span>
            </div>
            <div class="mt-2 text-xs font-mono break-all text-base-content/60">
                {item.secret_public_id.to_string()}
            </div>
            <div class="mt-3 flex flex-wrap gap-2">
                <Button onclick={on_use_secret}>{"Use for rotate/revoke"}</Button>
                <Button onclick={on_use_routing_secret}>{"Use for routing bind"}</Button>
                <Button onclick={on_use_field_secret}>{"Use for field bind"}</Button>
                <Button onclick={on_use_prowlarr_secret}>{"Use for Prowlarr"}</Button>
            </div>
        </div>
    }
}

#[function_component(TagSecretSection)]
fn tag_section(props: &TagSectionProps) -> Html {
    let on_tag_key = text_callback(props.draft.clone(), |draft, value| draft.tag_key = value);
    let on_tag_name = text_callback(props.draft.clone(), |draft, value| {
        draft.tag_display_name = value;
    });
    let on_tag_id = text_callback(props.draft.clone(), |draft, value| {
        draft.tag_public_id = value;
    });
    let tag_inventory = props.tags.items.iter().map(|item| {
        let draft = props.draft.clone();
        let item_for_use = item.clone();
        let on_use_tag = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.tag_public_id = item_for_use.tag_public_id.to_string();
            next.tag_key = item_for_use.tag_key.clone();
            next.tag_display_name = item_for_use.display_name.clone();
            draft.set(next);
        });
        let draft = props.draft.clone();
        let item_for_indexer = item.clone();
        let on_append_indexer_tag = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.indexer_tag_keys =
                append_csv_unique(&next.indexer_tag_keys, &item_for_indexer.tag_key);
            draft.set(next);
        });
        let draft = props.draft.clone();
        let item_for_allow = item.clone();
        let on_append_allow_tag = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.search_profile_tag_keys_allow =
                append_csv_unique(&next.search_profile_tag_keys_allow, &item_for_allow.tag_key);
            draft.set(next);
        });
        render_tag_inventory_item(item, on_use_tag, on_append_indexer_tag, on_append_allow_tag)
    });
    card(
        "Tags",
        html! {
            <div class="space-y-4">
                <div class="grid gap-4 lg:grid-cols-3">
                    {field("Tag key", "Lowercase key for search profile and instance bindings.", html! {
                        <Input value={props.draft.tag_key.clone()} oninput={on_tag_key} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Display name", "Human-friendly label shown in operators surfaces.", html! {
                        <Input value={props.draft.tag_display_name.clone()} oninput={on_tag_name} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Tag public ID", "Required for update/delete when the key is ambiguous.", html! {
                        <Input value={props.draft.tag_public_id.clone()} oninput={on_tag_id} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <div class="lg:col-span-3 flex flex-wrap gap-2">
                        <Button onclick={props.on_fetch_tags.clone()} disabled={props.busy}>{"Fetch tags"}</Button>
                        <Button onclick={props.on_primary.clone()} disabled={props.busy}>{"Create tag"}</Button>
                        <Button onclick={props.on_secondary.clone()} disabled={props.busy}>{"Update tag"}</Button>
                        <Button onclick={props.on_tertiary.clone()} disabled={props.busy}>{"Delete tag"}</Button>
                    </div>
                </div>
                {
                    if props.tags.items.is_empty() {
                        html! {
                            <p class="text-sm text-base-content/60">
                                {"Fetch tags to populate CRUD forms and append tag keys into search-profile or indexer bindings."}
                            </p>
                        }
                    } else {
                        html! {
                            <div class="space-y-3">
                                {for tag_inventory}
                            </div>
                        }
                    }
                }
            </div>
        },
    )
}

#[function_component(SecretSection)]
fn secret_section(props: &SecretSectionProps) -> Html {
    let on_secret_type = text_callback(props.draft.clone(), |draft, value| {
        draft.secret_type = value
    });
    let on_secret_name = text_callback(props.draft.clone(), |draft, value| {
        draft.secret_display_name = value;
    });
    let on_secret_value = text_callback(props.draft.clone(), |draft, value| {
        draft.secret_value = value
    });
    let on_secret_id = text_callback(props.draft.clone(), |draft, value| {
        draft.secret_public_id = value;
    });
    let on_secret_new_value = text_callback(props.draft.clone(), |draft, value| {
        draft.secret_new_value = value;
    });
    let secret_inventory = props.secrets.items.iter().map(|item| {
        let draft = props.draft.clone();
        let item_for_secret = item.clone();
        let on_use_secret = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.secret_public_id = item_for_secret.secret_public_id.to_string();
            next.secret_type = item_for_secret.secret_type.clone();
            draft.set(next);
        });
        let draft = props.draft.clone();
        let item_for_routing = item.clone();
        let on_use_routing_secret = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.routing_secret_public_id = item_for_routing.secret_public_id.to_string();
            draft.set(next);
        });
        let draft = props.draft.clone();
        let item_for_field = item.clone();
        let on_use_field_secret = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.indexer_field_secret_public_id = item_for_field.secret_public_id.to_string();
            draft.set(next);
        });
        let draft = props.draft.clone();
        let item_for_prowlarr = item.clone();
        let on_use_prowlarr_secret = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.prowlarr_api_key = item_for_prowlarr.secret_public_id.to_string();
            draft.set(next);
        });
        render_secret_inventory_item(
            item,
            on_use_secret,
            on_use_routing_secret,
            on_use_field_secret,
            on_use_prowlarr_secret,
        )
    });
    card(
        "Secrets",
        html! {
            <div class="space-y-4">
                <div class="grid gap-4 lg:grid-cols-3">
                    {field("Secret type", "api_key, password, cookie, token, or header_value.", html! {
                        <Input value={props.draft.secret_type.clone()} oninput={on_secret_type} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Display name", "Operator-facing label used in audit output.", html! {
                        <Input value={props.draft.secret_display_name.clone()} oninput={on_secret_name} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Secret public ID", "Used for rotation and revocation.", html! {
                        <Input value={props.draft.secret_public_id.clone()} oninput={on_secret_id} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Secret value", "Plaintext secret for initial create.", html! {
                        <Textarea value={props.draft.secret_value.clone()} rows={3} oninput={on_secret_value} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Rotated value", "Plaintext secret used when rotating an existing secret.", html! {
                        <Textarea value={props.draft.secret_new_value.clone()} rows={3} oninput={on_secret_new_value} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <div class="lg:col-span-3 flex flex-wrap gap-2">
                        <Button onclick={props.on_fetch_secrets.clone()} disabled={props.busy}>{"Fetch secrets"}</Button>
                        <Button onclick={props.on_primary.clone()} disabled={props.busy}>{"Create secret"}</Button>
                        <Button onclick={props.on_secondary.clone()} disabled={props.busy}>{"Rotate secret"}</Button>
                        <Button onclick={props.on_tertiary.clone()} disabled={props.busy}>{"Revoke secret"}</Button>
                    </div>
                </div>
                {
                    if props.secrets.items.is_empty() {
                        html! {
                            <p class="text-sm text-base-content/60">
                                {"Fetch secret metadata to populate rotation, binding, and Prowlarr API-key flows without manual UUID entry."}
                            </p>
                        }
                    } else {
                        html! {
                            <div class="space-y-3">
                                {for secret_inventory}
                            </div>
                        }
                    }
                }
            </div>
        },
    )
}

#[function_component(HealthNotificationsSection)]
fn health_notifications_section(props: &HealthNotificationsSectionProps) -> Html {
    let on_hook_id = text_callback(props.draft.clone(), |draft, value| {
        draft.health_notification_hook_public_id = value;
    });
    let on_channel = text_callback(props.draft.clone(), |draft, value| {
        draft.health_notification_channel = value;
    });
    let on_display_name = text_callback(props.draft.clone(), |draft, value| {
        draft.health_notification_display_name = value;
    });
    let on_threshold = text_callback(props.draft.clone(), |draft, value| {
        draft.health_notification_status_threshold = value;
    });
    let on_webhook_url = text_callback(props.draft.clone(), |draft, value| {
        draft.health_notification_webhook_url = value;
    });
    let on_email = text_callback(props.draft.clone(), |draft, value| {
        draft.health_notification_email = value;
    });
    let on_enabled = bool_callback(props.draft.clone(), |draft, value| {
        draft.health_notification_is_enabled = value;
    });
    card(
        "Health notifications",
        html! {
            <div class="space-y-4">
                <div class="grid gap-4 lg:grid-cols-3">
                    {field("Hook public ID", "Required for update/delete of an existing hook.", html! {
                        <Input value={props.draft.health_notification_hook_public_id.clone()} oninput={on_hook_id} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Channel", "email or webhook.", html! {
                        <Input value={props.draft.health_notification_channel.clone()} oninput={on_channel} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Display name", "Operator-facing label for the destination.", html! {
                        <Input value={props.draft.health_notification_display_name.clone()} oninput={on_display_name} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Trigger threshold", "degraded, failing, or quarantined.", html! {
                        <Input value={props.draft.health_notification_status_threshold.clone()} oninput={on_threshold} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Webhook URL", "Used when channel=webhook.", html! {
                        <Input value={props.draft.health_notification_webhook_url.clone()} oninput={on_webhook_url} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Email address", "Used when channel=email.", html! {
                        <Input value={props.draft.health_notification_email.clone()} oninput={on_email} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <label class="flex items-center gap-3 rounded-box border border-base-300 px-3 py-2 lg:col-span-3">
                        <input
                            type="checkbox"
                            class="checkbox"
                            checked={props.draft.health_notification_is_enabled}
                            onchange={on_enabled}
                            disabled={props.busy}
                        />
                        <span>{"Hook enabled"}</span>
                    </label>
                </div>
                <div class="flex flex-wrap gap-2">
                    <Button onclick={props.on_fetch_hooks.clone()} disabled={props.busy}>{"Fetch notification hooks"}</Button>
                    <Button onclick={props.on_create_hook.clone()} disabled={props.busy}>{"Create notification hook"}</Button>
                    <Button onclick={props.on_update_hook.clone()} disabled={props.busy}>{"Update notification hook"}</Button>
                    <Button onclick={props.on_delete_hook.clone()} disabled={props.busy}>{"Delete notification hook"}</Button>
                </div>
                {
                    if props.hooks.hooks.is_empty() {
                        html! {
                            <p class="text-sm text-base-content/60">
                                {"Fetch notification hooks to review the webhook and email destinations that should fire when indexers degrade or fail."}
                            </p>
                        }
                    } else {
                        html! {
                            <div class="space-y-3">
                                {for props.hooks.hooks.iter().map(render_health_notification_hook)}
                            </div>
                        }
                    }
                }
            </div>
        },
    )
}

#[derive(Properties, PartialEq)]
struct RoutingRateLimitSectionProps {
    draft: UseStateHandle<IndexersDraft>,
    routing_policy: RoutingPolicyState,
    routing_inventory: RoutingPolicyInventoryState,
    rate_limit_inventory: RateLimitInventoryState,
    busy: bool,
    on_fetch_routing_inventory: Callback<MouseEvent>,
    on_fetch_routing: Callback<MouseEvent>,
    on_create_routing: Callback<MouseEvent>,
    on_set_routing_param: Callback<MouseEvent>,
    on_bind_routing_secret: Callback<MouseEvent>,
    on_fetch_rate_limits: Callback<MouseEvent>,
    on_create_rate_limit: Callback<MouseEvent>,
    on_update_rate_limit: Callback<MouseEvent>,
    on_delete_rate_limit: Callback<MouseEvent>,
    on_assign_indexer: Callback<MouseEvent>,
    on_assign_routing: Callback<MouseEvent>,
}

#[function_component(RoutingRateLimitSection)]
fn routing_rate_limit_section(props: &RoutingRateLimitSectionProps) -> Html {
    let on_routing_name = text_callback(props.draft.clone(), |draft, value| {
        draft.routing_display_name = value;
    });
    let on_routing_mode = text_callback(props.draft.clone(), |draft, value| {
        draft.routing_mode = value;
    });
    let on_routing_id = text_callback(props.draft.clone(), |draft, value| {
        draft.routing_policy_public_id = value;
    });
    let on_routing_key = text_callback(props.draft.clone(), |draft, value| {
        draft.routing_param_key = value;
    });
    let on_routing_plain = text_callback(props.draft.clone(), |draft, value| {
        draft.routing_param_plain = value;
    });
    let on_routing_int = text_callback(props.draft.clone(), |draft, value| {
        draft.routing_param_int = value;
    });
    let on_routing_bool = text_callback(props.draft.clone(), |draft, value| {
        draft.routing_param_bool = value;
    });
    let on_routing_secret = text_callback(props.draft.clone(), |draft, value| {
        draft.routing_secret_public_id = value;
    });
    let on_rl_id = text_callback(props.draft.clone(), |draft, value| {
        draft.rate_limit_public_id = value;
    });
    let on_rl_name = text_callback(props.draft.clone(), |draft, value| {
        draft.rate_limit_display_name = value;
    });
    let on_rl_rpm = text_callback(props.draft.clone(), |draft, value| {
        draft.rate_limit_rpm = value
    });
    let on_rl_burst = text_callback(props.draft.clone(), |draft, value| {
        draft.rate_limit_burst = value;
    });
    let on_rl_concurrent = text_callback(props.draft.clone(), |draft, value| {
        draft.rate_limit_concurrent = value;
    });
    let on_rl_indexer = text_callback(props.draft.clone(), |draft, value| {
        draft.rate_limit_indexer_public_id = value;
    });
    let on_rl_routing = text_callback(props.draft.clone(), |draft, value| {
        draft.rate_limit_routing_public_id = value;
    });
    let routing_inventory = props.routing_inventory.items.iter().map(|item| {
        let draft = props.draft.clone();
        let routing_policy_public_id = item.routing_policy_public_id;
        let display_name = item.display_name.clone();
        let mode = item.mode.clone();
        let on_use_policy = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.routing_policy_public_id = routing_policy_public_id.to_string();
            next.routing_display_name = display_name.clone();
            next.routing_mode = mode.clone();
            draft.set(next);
        });

        let draft = props.draft.clone();
        let routing_policy_public_id = item.routing_policy_public_id;
        let on_assign_rate_limit = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.routing_policy_public_id = routing_policy_public_id.to_string();
            next.rate_limit_routing_public_id = routing_policy_public_id.to_string();
            draft.set(next);
        });

        render_routing_inventory_item(item, on_use_policy, on_assign_rate_limit)
    });
    let rate_limit_inventory = props.rate_limit_inventory.items.iter().map(|item| {
        let draft = props.draft.clone();
        let policy_public_id = item.rate_limit_policy_public_id;
        let display_name = item.display_name.clone();
        let rpm = item.requests_per_minute.to_string();
        let burst = item.burst.to_string();
        let concurrent = item.concurrent_requests.to_string();
        let on_use_policy = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.rate_limit_public_id = policy_public_id.to_string();
            next.rate_limit_display_name = display_name.clone();
            next.rate_limit_rpm = rpm.clone();
            next.rate_limit_burst = burst.clone();
            next.rate_limit_concurrent = concurrent.clone();
            draft.set(next);
        });

        let draft = props.draft.clone();
        let policy_public_id = item.rate_limit_policy_public_id;
        let on_assign_indexer = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.rate_limit_public_id = policy_public_id.to_string();
            draft.set(next);
        });

        let draft = props.draft.clone();
        let policy_public_id = item.rate_limit_policy_public_id;
        let on_assign_routing = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.rate_limit_public_id = policy_public_id.to_string();
            draft.set(next);
        });

        render_rate_limit_inventory_item(item, on_use_policy, on_assign_indexer, on_assign_routing)
    });
    card(
        "Routing and rate limits",
        html! {
            <div class="grid gap-4 xl:grid-cols-2">
                <div class="space-y-4 rounded-box border border-base-300 p-4">
                    <h3 class="font-semibold">{"Routing policies"}</h3>
                    {field("Display name", "Human label for the routing policy.", html! {
                        <Input value={props.draft.routing_display_name.clone()} oninput={on_routing_name} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Mode", "direct, http_proxy, socks_proxy, flaresolverr, vpn_route, or tor.", html! {
                        <Input value={props.draft.routing_mode.clone()} oninput={on_routing_mode} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Routing policy public ID", "Target existing policy for param/secret/rate-limit updates.", html! {
                        <Input value={props.draft.routing_policy_public_id.clone()} oninput={on_routing_id} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Param key", "Examples: proxy_host, proxy_port, verify_tls, fs_url.", html! {
                        <Input value={props.draft.routing_param_key.clone()} oninput={on_routing_key} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <div class="grid gap-3 md:grid-cols-3">
                        {field("Value (plain)", "Optional text parameter.", html! {
                            <Input value={props.draft.routing_param_plain.clone()} oninput={on_routing_plain} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Value (int)", "Optional integer parameter.", html! {
                            <Input value={props.draft.routing_param_int.clone()} oninput={on_routing_int} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Value (bool)", "Optional bool text: true or false.", html! {
                            <Input value={props.draft.routing_param_bool.clone()} oninput={on_routing_bool} disabled={props.busy} class={classes!("w-full")} />
                        })}
                    </div>
                    {field("Secret public ID", "Bind an existing secret to the routing parameter.", html! {
                        <Input value={props.draft.routing_secret_public_id.clone()} oninput={on_routing_secret} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <div class="flex flex-wrap gap-2">
                        <Button onclick={props.on_fetch_routing_inventory.clone()} disabled={props.busy}>{"Fetch routing inventory"}</Button>
                        <Button onclick={props.on_fetch_routing.clone()} disabled={props.busy}>{"Fetch routing policy"}</Button>
                        <Button onclick={props.on_create_routing.clone()} disabled={props.busy}>{"Create routing policy"}</Button>
                        <Button onclick={props.on_set_routing_param.clone()} disabled={props.busy}>{"Set parameter"}</Button>
                        <Button onclick={props.on_bind_routing_secret.clone()} disabled={props.busy}>{"Bind secret"}</Button>
                    </div>
                    {
                        if props.routing_inventory.items.is_empty() {
                            Html::default()
                        } else {
                            html! { <div class="grid gap-3">{for routing_inventory}</div> }
                        }
                    }
                    {
                        if let Some(detail) = props.routing_policy.detail.as_ref() {
                            render_routing_policy_detail(detail)
                        } else {
                            html! {
                                <p class="text-sm text-base-content/60">
                                    {"Fetch a routing policy to review per-indexer proxy, flaresolverr, and secret bindings."}
                                </p>
                            }
                        }
                    }
                </div>
                <div class="space-y-4 rounded-box border border-base-300 p-4">
                    <h3 class="font-semibold">{"Rate-limit policies"}</h3>
                    {field("Rate-limit public ID", "Existing policy for update/delete/assignment.", html! {
                        <Input value={props.draft.rate_limit_public_id.clone()} oninput={on_rl_id} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Display name", "Operator-facing policy label.", html! {
                        <Input value={props.draft.rate_limit_display_name.clone()} oninput={on_rl_name} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <div class="grid gap-3 md:grid-cols-3">
                        {field("RPM", "Requests per minute.", html! {
                            <Input value={props.draft.rate_limit_rpm.clone()} oninput={on_rl_rpm} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Burst", "Bucket burst size.", html! {
                            <Input value={props.draft.rate_limit_burst.clone()} oninput={on_rl_burst} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Concurrent", "Max in-flight requests.", html! {
                            <Input value={props.draft.rate_limit_concurrent.clone()} oninput={on_rl_concurrent} disabled={props.busy} class={classes!("w-full")} />
                        })}
                    </div>
                    <div class="grid gap-3 md:grid-cols-2">
                        {field("Indexer public ID", "Assign the rate-limit policy to an indexer instance.", html! {
                            <Input value={props.draft.rate_limit_indexer_public_id.clone()} oninput={on_rl_indexer} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Routing public ID", "Assign the rate-limit policy to a routing policy.", html! {
                            <Input value={props.draft.rate_limit_routing_public_id.clone()} oninput={on_rl_routing} disabled={props.busy} class={classes!("w-full")} />
                        })}
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button onclick={props.on_fetch_rate_limits.clone()} disabled={props.busy}>{"Fetch rate limits"}</Button>
                        <Button onclick={props.on_create_rate_limit.clone()} disabled={props.busy}>{"Create rate limit"}</Button>
                        <Button onclick={props.on_update_rate_limit.clone()} disabled={props.busy}>{"Update rate limit"}</Button>
                        <Button onclick={props.on_delete_rate_limit.clone()} disabled={props.busy}>{"Delete rate limit"}</Button>
                        <Button onclick={props.on_assign_indexer.clone()} disabled={props.busy}>{"Assign to indexer"}</Button>
                        <Button onclick={props.on_assign_routing.clone()} disabled={props.busy}>{"Assign to routing"}</Button>
                    </div>
                    {
                        if props.rate_limit_inventory.items.is_empty() {
                            Html::default()
                        } else {
                            html! { <div class="grid gap-3">{for rate_limit_inventory}</div> }
                        }
                    }
                </div>
            </div>
        },
    )
}

#[derive(Properties, PartialEq)]
struct InstancesSectionProps {
    draft: UseStateHandle<IndexersDraft>,
    instances: IndexerInstanceInventoryState,
    connectivity: ConnectivityInsightsState,
    health_events: HealthEventsState,
    busy: bool,
    on_fetch_instances: Callback<MouseEvent>,
    on_create_instance: Callback<MouseEvent>,
    on_update_instance: Callback<MouseEvent>,
    on_fetch_rss_subscription: Callback<MouseEvent>,
    on_update_rss_subscription: Callback<MouseEvent>,
    on_fetch_rss_items: Callback<MouseEvent>,
    on_mark_rss_item_seen: Callback<MouseEvent>,
    on_fetch_connectivity_profile: Callback<MouseEvent>,
    on_fetch_health_events: Callback<MouseEvent>,
    on_fetch_source_reputation: Callback<MouseEvent>,
    on_set_media_domains: Callback<MouseEvent>,
    on_set_tags: Callback<MouseEvent>,
    on_upsert_tracker_category_mapping: Callback<MouseEvent>,
    on_delete_tracker_category_mapping: Callback<MouseEvent>,
    on_set_field_value: Callback<MouseEvent>,
    on_bind_field_secret: Callback<MouseEvent>,
    on_fetch_cf_state: Callback<MouseEvent>,
    on_reset_cf_state: Callback<MouseEvent>,
    on_prepare_test: Callback<MouseEvent>,
    on_finalize_test: Callback<MouseEvent>,
}

#[function_component(InstancesSection)]
fn instances_section(props: &InstancesSectionProps) -> Html {
    let on_definition = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_definition_upstream_slug = value;
    });
    let on_display_name = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_display_name = value;
    });
    let on_priority = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_priority = value;
    });
    let on_trust_tier = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_trust_tier_key = value;
    });
    let on_routing = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_routing_policy_public_id = value;
    });
    let on_instance_id = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_instance_public_id = value;
    });
    let on_instance_enabled = bool_callback(props.draft.clone(), |draft, value| {
        draft.indexer_is_enabled = value;
    });
    let on_instance_rss = bool_callback(props.draft.clone(), |draft, value| {
        draft.indexer_enable_rss = value;
    });
    let on_instance_auto = bool_callback(props.draft.clone(), |draft, value| {
        draft.indexer_enable_automatic_search = value;
    });
    let on_instance_interactive = bool_callback(props.draft.clone(), |draft, value| {
        draft.indexer_enable_interactive_search = value;
    });
    let on_rss_interval = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_rss_interval_seconds = value;
    });
    let on_rss_recent_limit = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_rss_recent_limit = value;
    });
    let on_reputation_window = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_reputation_window = value;
    });
    let on_reputation_limit = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_reputation_limit = value;
    });
    let on_health_event_limit = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_health_event_limit = value;
    });
    let on_rss_item_guid = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_rss_item_guid = value;
    });
    let on_rss_infohash_v1 = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_rss_infohash_v1 = value;
    });
    let on_rss_infohash_v2 = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_rss_infohash_v2 = value;
    });
    let on_rss_magnet_hash = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_rss_magnet_hash = value;
    });
    let on_domains = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_media_domain_keys = value;
    });
    let on_tags = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_tag_keys = value;
    });
    let on_category_mapping_definition = text_callback(props.draft.clone(), |draft, value| {
        draft.category_mapping_definition_upstream_slug = value;
    });
    let on_category_mapping_torznab_instance =
        text_callback(props.draft.clone(), |draft, value| {
            draft.category_mapping_torznab_instance_public_id = value;
        });
    let on_category_mapping_instance = text_callback(props.draft.clone(), |draft, value| {
        draft.category_mapping_indexer_instance_public_id = value;
    });
    let on_category_mapping_tracker = text_callback(props.draft.clone(), |draft, value| {
        draft.category_mapping_tracker_category = value;
    });
    let on_category_mapping_subcategory = text_callback(props.draft.clone(), |draft, value| {
        draft.category_mapping_tracker_subcategory = value;
    });
    let on_category_mapping_torznab = text_callback(props.draft.clone(), |draft, value| {
        draft.category_mapping_torznab_cat_id = value;
    });
    let on_category_mapping_media_domain = text_callback(props.draft.clone(), |draft, value| {
        draft.category_mapping_media_domain_key = value;
    });
    let on_field_name = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_field_name = value;
    });
    let on_field_plain = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_field_plain = value;
    });
    let on_field_int = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_field_int = value;
    });
    let on_field_decimal = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_field_decimal = value;
    });
    let on_field_bool = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_field_bool = value;
    });
    let on_field_secret = text_callback(props.draft.clone(), |draft, value| {
        draft.indexer_field_secret_public_id = value;
    });
    let on_cf_reason = text_callback(props.draft.clone(), |draft, value| {
        draft.cf_reset_reason = value;
    });
    let on_test_ok = bool_callback(props.draft.clone(), |draft, value| draft.test_ok = value);
    let on_test_class = text_callback(props.draft.clone(), |draft, value| {
        draft.test_error_class = value;
    });
    let on_test_code = text_callback(props.draft.clone(), |draft, value| {
        draft.test_error_code = value;
    });
    let on_test_detail = text_callback(props.draft.clone(), |draft, value| {
        draft.test_detail = value;
    });
    let on_test_count = text_callback(props.draft.clone(), |draft, value| {
        draft.test_result_count = value;
    });
    let instance_inventory = props.instances.items.iter().map(|item| {
        let draft = props.draft.clone();
        let item_clone = item.clone();
        let on_use_instance = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.indexer_instance_public_id = item_clone.indexer_instance_public_id.to_string();
            next.indexer_definition_upstream_slug = item_clone.upstream_slug.clone();
            next.indexer_display_name = item_clone.display_name.clone();
            next.indexer_priority = item_clone.priority.to_string();
            next.indexer_trust_tier_key = item_clone.trust_tier_key.clone().unwrap_or_default();
            next.indexer_routing_policy_public_id = item_clone
                .routing_policy_public_id
                .map(|value| value.to_string())
                .unwrap_or_default();
            next.indexer_is_enabled = item_clone.instance_status == "enabled";
            next.indexer_enable_rss = item_clone.rss_status == "enabled";
            next.indexer_enable_automatic_search = item_clone.automatic_search_status == "enabled";
            next.indexer_enable_interactive_search =
                item_clone.interactive_search_status == "enabled";
            next.indexer_media_domain_keys = item_clone.media_domain_keys.join(", ");
            next.indexer_tag_keys = item_clone.tag_keys.join(", ");
            draft.set(next);
        });

        let draft = props.draft.clone();
        let item_clone = item.clone();
        let on_use_rss = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.indexer_instance_public_id = item_clone.indexer_instance_public_id.to_string();
            next.indexer_rss_interval_seconds = item_clone
                .rss_interval_seconds
                .map(|value| value.to_string())
                .unwrap_or_else(|| "900".to_string());
            draft.set(next);
        });

        let draft = props.draft.clone();
        let item_clone = item.clone();
        let on_use_assignment = Callback::from(move |_| {
            let mut next = (*draft).clone();
            next.indexer_instance_public_id = item_clone.indexer_instance_public_id.to_string();
            next.rate_limit_indexer_public_id = item_clone.indexer_instance_public_id.to_string();
            draft.set(next);
        });

        render_indexer_instance_inventory_item(item, on_use_instance, on_use_rss, on_use_assignment)
    });
    card(
        "Indexer instances",
        html! {
            <div class="space-y-4">
                <div class="flex flex-wrap gap-2">
                    <Button onclick={props.on_fetch_instances.clone()} disabled={props.busy}>{"Fetch instances"}</Button>
                </div>
                {
                    if props.instances.items.is_empty() {
                        Html::default()
                    } else {
                        html! { <div class="grid gap-3">{for instance_inventory}</div> }
                    }
                }
                <div class="grid gap-4 xl:grid-cols-3">
                    {field("Definition upstream slug", "Use a catalog slug from the catalog section above.", html! {
                        <Input value={props.draft.indexer_definition_upstream_slug.clone()} oninput={on_definition} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Display name", "Human-friendly indexer instance name.", html! {
                        <Input value={props.draft.indexer_display_name.clone()} oninput={on_display_name} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Instance public ID", "Use for updates, field edits, CF checks, and tests.", html! {
                        <Input value={props.draft.indexer_instance_public_id.clone()} oninput={on_instance_id} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Priority", "Optional integer priority override.", html! {
                        <Input value={props.draft.indexer_priority.clone()} oninput={on_priority} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Trust tier key", "public, semi_private, private, or invite_only.", html! {
                        <Input value={props.draft.indexer_trust_tier_key.clone()} oninput={on_trust_tier} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Routing policy public ID", "Optional routing policy binding for create/update.", html! {
                        <Input value={props.draft.indexer_routing_policy_public_id.clone()} oninput={on_routing} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <div class="xl:col-span-3 grid gap-3 md:grid-cols-2 xl:grid-cols-4">
                        <label class="flex items-center gap-3 rounded-box border border-base-300 px-3 py-2">
                            <input
                                type="checkbox"
                                class="checkbox"
                                checked={props.draft.indexer_is_enabled}
                                onchange={on_instance_enabled}
                                disabled={props.busy}
                            />
                            <span>{"Indexer enabled"}</span>
                        </label>
                        <label class="flex items-center gap-3 rounded-box border border-base-300 px-3 py-2">
                            <input
                                type="checkbox"
                                class="checkbox"
                                checked={props.draft.indexer_enable_rss}
                                onchange={on_instance_rss}
                                disabled={props.busy}
                            />
                            <span>{"RSS enabled"}</span>
                        </label>
                        <label class="flex items-center gap-3 rounded-box border border-base-300 px-3 py-2">
                            <input
                                type="checkbox"
                                class="checkbox"
                                checked={props.draft.indexer_enable_automatic_search}
                                onchange={on_instance_auto}
                                disabled={props.busy}
                            />
                            <span>{"Automatic search enabled"}</span>
                        </label>
                        <label class="flex items-center gap-3 rounded-box border border-base-300 px-3 py-2">
                            <input
                                type="checkbox"
                                class="checkbox"
                                checked={props.draft.indexer_enable_interactive_search}
                                onchange={on_instance_interactive}
                                disabled={props.busy}
                            />
                            <span>{"Interactive search enabled"}</span>
                        </label>
                    </div>
                    {field("Media domain keys", "Comma or newline separated: movies, tv, ebooks, software, etc.", html! {
                        <Textarea value={props.draft.indexer_media_domain_keys.clone()} rows={3} oninput={on_domains} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Tag keys", "Comma or newline separated keys.", html! {
                        <Textarea value={props.draft.indexer_tag_keys.clone()} rows={3} oninput={on_tags} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <div class="xl:col-span-3 space-y-3 rounded-box border border-base-300 p-4">
                        <h3 class="font-semibold">{"Category overrides"}</h3>
                        <p class="text-sm text-base-content/70">
                            {"Store tracker-to-Torznab mappings globally, per downstream app Torznab instance, per definition, or per indexer instance. App-scoped overrides let one sync target publish different categories without changing the shared default mapping."}
                        </p>
                        <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                            {field("Override app Torznab instance", "Optional Torznab instance public ID for app-scoped overrides.", html! {
                                <Input value={props.draft.category_mapping_torznab_instance_public_id.clone()} oninput={on_category_mapping_torznab_instance} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Override definition slug", "Optional definition-level scope.", html! {
                                <Input value={props.draft.category_mapping_definition_upstream_slug.clone()} oninput={on_category_mapping_definition} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Override instance public ID", "Optional instance-level scope. Leave blank for app-wide or global mappings.", html! {
                                <Input value={props.draft.category_mapping_indexer_instance_public_id.clone()} oninput={on_category_mapping_instance} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Media domain key", "Optional domain filter such as movies or tv.", html! {
                                <Input value={props.draft.category_mapping_media_domain_key.clone()} oninput={on_category_mapping_media_domain} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Tracker category", "Required tracker category integer.", html! {
                                <Input value={props.draft.category_mapping_tracker_category.clone()} oninput={on_category_mapping_tracker} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Tracker subcategory", "Optional tracker subcategory integer; defaults to 0.", html! {
                                <Input value={props.draft.category_mapping_tracker_subcategory.clone()} oninput={on_category_mapping_subcategory} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Torznab category", "Required Torznab category integer for upserts.", html! {
                                <Input value={props.draft.category_mapping_torznab_cat_id.clone()} oninput={on_category_mapping_torznab} disabled={props.busy} class={classes!("w-full")} />
                            })}
                        </div>
                        <div class="flex flex-wrap gap-2">
                            <Button onclick={props.on_upsert_tracker_category_mapping.clone()} disabled={props.busy}>{"Upsert tracker mapping"}</Button>
                            <Button onclick={props.on_delete_tracker_category_mapping.clone()} disabled={props.busy}>{"Delete tracker mapping"}</Button>
                        </div>
                    </div>
                    {field("Cloudflare reset reason", "Stored reason string for manual resets.", html! {
                        <Input value={props.draft.cf_reset_reason.clone()} oninput={on_cf_reason} disabled={props.busy} class={classes!("w-full")} />
                    })}
                </div>
                <div class="flex flex-wrap gap-2">
                    <Button onclick={props.on_create_instance.clone()} disabled={props.busy}>{"Create instance"}</Button>
                    <Button onclick={props.on_update_instance.clone()} disabled={props.busy}>{"Update instance"}</Button>
                    <Button onclick={props.on_set_media_domains.clone()} disabled={props.busy}>{"Set media domains"}</Button>
                    <Button onclick={props.on_set_tags.clone()} disabled={props.busy}>{"Set tags"}</Button>
                    <Button onclick={props.on_fetch_cf_state.clone()} disabled={props.busy}>{"Fetch CF state"}</Button>
                    <Button onclick={props.on_reset_cf_state.clone()} disabled={props.busy}>{"Reset CF state"}</Button>
                </div>
                <div class="grid gap-4 xl:grid-cols-2">
                    <div class="space-y-4 rounded-box border border-base-300 p-4">
                        <h3 class="font-semibold">{"RSS management"}</h3>
                        <div class="grid gap-3 md:grid-cols-2">
                            {field("RSS interval seconds", "Stored subscription cadence. Accepts 300 to 86400.", html! {
                                <Input value={props.draft.indexer_rss_interval_seconds.clone()} oninput={on_rss_interval} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Recent item limit", "How many recent seen items to fetch for review.", html! {
                                <Input value={props.draft.indexer_rss_recent_limit.clone()} oninput={on_rss_recent_limit} disabled={props.busy} class={classes!("w-full")} />
                            })}
                        </div>
                        <div class="flex flex-wrap gap-2">
                            <Button onclick={props.on_fetch_rss_subscription.clone()} disabled={props.busy}>{"Fetch RSS status"}</Button>
                            <Button onclick={props.on_update_rss_subscription.clone()} disabled={props.busy}>{"Update RSS subscription"}</Button>
                            <Button onclick={props.on_fetch_rss_items.clone()} disabled={props.busy}>{"Fetch recent RSS items"}</Button>
                        </div>
                        <div class="grid gap-3 md:grid-cols-2">
                            {field("Item GUID", "Optional stable GUID or feed item ID.", html! {
                                <Input value={props.draft.indexer_rss_item_guid.clone()} oninput={on_rss_item_guid} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Infohash v1", "Optional 40-char lowercase or uppercase hex.", html! {
                                <Input value={props.draft.indexer_rss_infohash_v1.clone()} oninput={on_rss_infohash_v1} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Infohash v2", "Optional 64-char lowercase or uppercase hex.", html! {
                                <Input value={props.draft.indexer_rss_infohash_v2.clone()} oninput={on_rss_infohash_v2} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Magnet hash", "Optional 64-char hash for magnet-only items.", html! {
                                <Input value={props.draft.indexer_rss_magnet_hash.clone()} oninput={on_rss_magnet_hash} disabled={props.busy} class={classes!("w-full")} />
                            })}
                        </div>
                        <div class="flex flex-wrap gap-2">
                            <Button onclick={props.on_mark_rss_item_seen.clone()} disabled={props.busy}>{"Mark RSS item seen"}</Button>
                        </div>
                    </div>
                    <div class="space-y-4 rounded-box border border-base-300 p-4">
                        <h3 class="font-semibold">{"Connectivity & reputation"}</h3>
                        <div class="grid gap-3 md:grid-cols-2">
                            {field("Reputation window", "1h, 24h, or 7d.", html! {
                                <Input value={props.draft.indexer_reputation_window.clone()} oninput={on_reputation_window} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Reputation limit", "Recent rows to fetch for the selected window.", html! {
                                <Input value={props.draft.indexer_reputation_limit.clone()} oninput={on_reputation_limit} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Health-event limit", "How many recent raw health events to fetch for drill-down.", html! {
                                <Input value={props.draft.indexer_health_event_limit.clone()} oninput={on_health_event_limit} disabled={props.busy} class={classes!("w-full")} />
                            })}
                        </div>
                        <div class="flex flex-wrap gap-2">
                            <Button onclick={props.on_fetch_connectivity_profile.clone()} disabled={props.busy}>{"Fetch connectivity profile"}</Button>
                            <Button onclick={props.on_fetch_source_reputation.clone()} disabled={props.busy}>{"Fetch source reputation"}</Button>
                            <Button onclick={props.on_fetch_health_events.clone()} disabled={props.busy}>{"Fetch health events"}</Button>
                            <Button onclick={props.on_fetch_cf_state.clone()} disabled={props.busy}>{"Fetch CF state"}</Button>
                            <Button onclick={props.on_reset_cf_state.clone()} disabled={props.busy}>{"Reset CF state"}</Button>
                        </div>
                        <div class="space-y-3">
                            <h4 class="text-sm font-semibold">{"Connectivity profile"}</h4>
                            {
                                if let Some(profile) = props.connectivity.profile.as_ref() {
                                    render_connectivity_profile(profile)
                                } else {
                                    html! {
                                        <p class="text-sm text-base-content/60">
                                            {"Fetch the derived connectivity profile to review current status, dominant failures, and latency bands."}
                                        </p>
                                    }
                                }
                            }
                        </div>
                        <div class="space-y-3">
                            <h4 class="text-sm font-semibold">{"Source reputation"}</h4>
                            {
                                if props.connectivity.reputation_items.is_empty() {
                                    html! {
                                        <p class="text-sm text-base-content/60">
                                            {"Fetch source reputation to compare request and acquisition outcomes across the selected time window."}
                                        </p>
                                    }
                                } else {
                                    html! {
                                        <div class="space-y-3">
                                            {for props.connectivity.reputation_items.iter().map(render_source_reputation)}
                                        </div>
                                    }
                                }
                            }
                        </div>
                        <div class="space-y-3">
                            <h4 class="text-sm font-semibold">{"Health events"}</h4>
                            {
                                if props.health_events.items.is_empty() {
                                    html! {
                                        <p class="text-sm text-base-content/60">
                                            {"Fetch health events to review recent diagnostic failures and conflict records for the selected indexer."}
                                        </p>
                                    }
                                } else {
                                    html! {
                                        <div class="space-y-3">
                                            {for props.health_events.items.iter().map(render_health_event)}
                                        </div>
                                    }
                                }
                            }
                        </div>
                    </div>
                    <div class="space-y-4 rounded-box border border-base-300 p-4">
                        <h3 class="font-semibold">{"Field values and secrets"}</h3>
                        {field("Field name", "Definition field key to update or bind.", html! {
                            <Input value={props.draft.indexer_field_name.clone()} oninput={on_field_name} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        <div class="grid gap-3 md:grid-cols-2">
                            {field("Plain value", "Optional text field payload.", html! {
                                <Input value={props.draft.indexer_field_plain.clone()} oninput={on_field_plain} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Integer value", "Optional integer field payload.", html! {
                                <Input value={props.draft.indexer_field_int.clone()} oninput={on_field_int} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Decimal value", "Optional decimal field payload as text.", html! {
                                <Input value={props.draft.indexer_field_decimal.clone()} oninput={on_field_decimal} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Bool value", "Optional bool text: true or false.", html! {
                                <Input value={props.draft.indexer_field_bool.clone()} oninput={on_field_bool} disabled={props.busy} class={classes!("w-full")} />
                            })}
                        </div>
                        {field("Field secret public ID", "Secret bound to the selected field name.", html! {
                            <Input value={props.draft.indexer_field_secret_public_id.clone()} oninput={on_field_secret} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        <div class="flex flex-wrap gap-2">
                            <Button onclick={props.on_set_field_value.clone()} disabled={props.busy}>{"Set field value"}</Button>
                            <Button onclick={props.on_bind_field_secret.clone()} disabled={props.busy}>{"Bind field secret"}</Button>
                        </div>
                    </div>
                    <div class="space-y-4 rounded-box border border-base-300 p-4">
                        <h3 class="font-semibold">{"Connectivity test finalization"}</h3>
                        <label class="flex items-center gap-3">
                            <input
                                type="checkbox"
                                class="checkbox"
                                checked={props.draft.test_ok}
                                onchange={on_test_ok}
                                disabled={props.busy}
                            />
                            <span>{"Mark finalize payload as successful"}</span>
                        </label>
                        {field("Error class", "Optional class such as auth_error, rate_limited, cf_challenge.", html! {
                            <Input value={props.draft.test_error_class.clone()} oninput={on_test_class} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Error code", "Optional implementation-specific error code.", html! {
                            <Input value={props.draft.test_error_code.clone()} oninput={on_test_code} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Detail", "Short operator-visible detail string.", html! {
                            <Textarea value={props.draft.test_detail.clone()} rows={3} oninput={on_test_detail} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Result count", "Optional diagnostic result count.", html! {
                            <Input value={props.draft.test_result_count.clone()} oninput={on_test_count} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        <div class="flex flex-wrap gap-2">
                            <Button onclick={props.on_prepare_test.clone()} disabled={props.busy}>{"Prepare test"}</Button>
                            <Button onclick={props.on_finalize_test.clone()} disabled={props.busy}>{"Finalize test"}</Button>
                        </div>
                    </div>
                </div>
            </div>
        },
    )
}

#[function_component(ProfilesPoliciesSection)]
fn profiles_policies_section(props: &ProfilesPoliciesSectionProps) -> Html {
    let on_profile_name = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_display_name = value;
    });
    let on_profile_id = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_public_id = value;
    });
    let on_profile_default = bool_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_is_default = value;
    });
    let on_profile_page_size = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_page_size = value;
    });
    let on_profile_default_domain = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_default_media_domain_key = value;
    });
    let on_profile_domains = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_media_domain_keys = value;
    });
    let on_profile_policy_set = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_policy_set_public_id = value;
    });
    let on_profile_indexers = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_indexer_public_ids = value;
    });
    let on_profile_allow_tags = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_tag_keys_allow = value;
    });
    let on_profile_block_tags = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_tag_keys_block = value;
    });
    let on_profile_prefer_tags = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_tag_keys_prefer = value;
    });
    let on_policy_set_name = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_set_display_name = value;
    });
    let on_policy_scope = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_set_scope = value;
    });
    let on_policy_set_id = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_set_public_id = value;
    });
    let on_policy_user = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_set_user_public_id = value;
    });
    let on_rule_type = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_rule_type = value;
    });
    let on_match_field = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_match_field = value;
    });
    let on_match_operator = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_match_operator = value;
    });
    let on_sort_order = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_sort_order = value;
    });
    let on_match_text = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_match_value_text = value;
    });
    let on_match_int = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_match_value_int = value;
    });
    let on_match_uuid = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_match_value_uuid = value;
    });
    let on_action = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_action = value
    });
    let on_severity = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_severity = value;
    });
    let on_case = bool_callback(props.draft.clone(), |draft, value| {
        draft.policy_is_case_insensitive = value;
    });
    let on_rationale = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_rationale = value;
    });
    let on_value_set = text_callback(props.draft.clone(), |draft, value| {
        draft.policy_value_set_text = value;
    });
    card(
        "Search profiles and policies",
        html! {
            <div class="grid gap-4 xl:grid-cols-2">
                <div class="space-y-4 rounded-box border border-base-300 p-4">
                    <h3 class="font-semibold">{"Search profiles"}</h3>
                    {field("Display name", "Create or rename a profile.", html! {
                        <Input value={props.draft.search_profile_display_name.clone()} oninput={on_profile_name} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Search profile public ID", "Use for update and assignment actions.", html! {
                        <Input value={props.draft.search_profile_public_id.clone()} oninput={on_profile_id} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <label class="flex items-center gap-3">
                        <input
                            type="checkbox"
                            class="checkbox"
                            checked={props.draft.search_profile_is_default}
                            onchange={on_profile_default}
                            disabled={props.busy}
                        />
                        <span>{"Mark created profile as default"}</span>
                    </label>
                    {field("Page size", "Optional page-size override.", html! {
                        <Input value={props.draft.search_profile_page_size.clone()} oninput={on_profile_page_size} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Default media domain", "Optional domain key for matching searches.", html! {
                        <Input value={props.draft.search_profile_default_media_domain_key.clone()} oninput={on_profile_default_domain} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Media domain allow-list", "Comma or newline separated domain keys.", html! {
                        <Textarea value={props.draft.search_profile_media_domain_keys.clone()} rows={3} oninput={on_profile_domains} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Policy-set public ID", "Attach an existing policy set to the profile.", html! {
                        <Input value={props.draft.search_profile_policy_set_public_id.clone()} oninput={on_profile_policy_set} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Indexer public IDs", "Comma or newline separated instance UUIDs for allow/block lists.", html! {
                        <Textarea value={props.draft.search_profile_indexer_public_ids.clone()} rows={3} oninput={on_profile_indexers} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <div class="grid gap-3 md:grid-cols-3">
                        {field("Allow tags", "Comma or newline separated tag keys.", html! {
                            <Textarea value={props.draft.search_profile_tag_keys_allow.clone()} rows={3} oninput={on_profile_allow_tags} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Block tags", "Comma or newline separated tag keys.", html! {
                            <Textarea value={props.draft.search_profile_tag_keys_block.clone()} rows={3} oninput={on_profile_block_tags} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Prefer tags", "Comma or newline separated tag keys.", html! {
                            <Textarea value={props.draft.search_profile_tag_keys_prefer.clone()} rows={3} oninput={on_profile_prefer_tags} disabled={props.busy} class={classes!("w-full")} />
                        })}
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button onclick={props.on_create_profile.clone()} disabled={props.busy}>{"Create profile"}</Button>
                        <Button onclick={props.on_update_profile.clone()} disabled={props.busy}>{"Update profile"}</Button>
                        <Button onclick={props.on_default_domain.clone()} disabled={props.busy}>{"Set default domain"}</Button>
                        <Button onclick={props.on_media_domains.clone()} disabled={props.busy}>{"Set media domains"}</Button>
                        <Button onclick={props.on_add_policy_set.clone()} disabled={props.busy}>{"Attach policy set"}</Button>
                        <Button onclick={props.on_allow_indexers.clone()} disabled={props.busy}>{"Allow indexers"}</Button>
                        <Button onclick={props.on_block_indexers.clone()} disabled={props.busy}>{"Block indexers"}</Button>
                        <Button onclick={props.on_allow_tags.clone()} disabled={props.busy}>{"Allow tags"}</Button>
                        <Button onclick={props.on_block_tags.clone()} disabled={props.busy}>{"Block tags"}</Button>
                        <Button onclick={props.on_prefer_tags.clone()} disabled={props.busy}>{"Prefer tags"}</Button>
                    </div>
                </div>
                <div class="space-y-4 rounded-box border border-base-300 p-4">
                    <h3 class="font-semibold">{"Policy sets and rules"}</h3>
                    {field("Policy-set display name", "Name for the policy set.", html! {
                        <Input value={props.draft.policy_set_display_name.clone()} oninput={on_policy_set_name} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <div class="grid gap-3 md:grid-cols-3">
                        {field("Scope", "global, user, profile, or request.", html! {
                            <Input value={props.draft.policy_set_scope.clone()} oninput={on_policy_scope} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Policy-set public ID", "Existing policy set for rule creation.", html! {
                            <Input value={props.draft.policy_set_public_id.clone()} oninput={on_policy_set_id} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("User public ID", "Optional UUID for user-scoped policy sets.", html! {
                            <Input value={props.draft.policy_set_user_public_id.clone()} oninput={on_policy_user} disabled={props.busy} class={classes!("w-full")} />
                        })}
                    </div>
                    <div class="grid gap-3 md:grid-cols-2">
                        {field("Rule type", "Examples: allow_title_regex, block_indexer_instance, prefer_trust_tier.", html! {
                            <Input value={props.draft.policy_rule_type.clone()} oninput={on_rule_type} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Match field", "Examples: title, tracker, indexer_instance_public_id.", html! {
                            <Input value={props.draft.policy_match_field.clone()} oninput={on_match_field} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Match operator", "eq, contains, regex, starts_with, ends_with, or in_set.", html! {
                            <Input value={props.draft.policy_match_operator.clone()} oninput={on_match_operator} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Sort order", "Lower values run earlier.", html! {
                            <Input value={props.draft.policy_sort_order.clone()} oninput={on_sort_order} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Action", "drop_source, downrank, require, prefer, or flag.", html! {
                            <Input value={props.draft.policy_action.clone()} oninput={on_action} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Severity", "hard or soft.", html! {
                            <Input value={props.draft.policy_severity.clone()} oninput={on_severity} disabled={props.busy} class={classes!("w-full")} />
                        })}
                    </div>
                    <div class="grid gap-3 md:grid-cols-3">
                        {field("Match text", "Optional text comparison value.", html! {
                            <Input value={props.draft.policy_match_value_text.clone()} oninput={on_match_text} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Match int", "Optional integer comparison value.", html! {
                            <Input value={props.draft.policy_match_value_int.clone()} oninput={on_match_int} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Match UUID", "Optional UUID comparison value.", html! {
                            <Input value={props.draft.policy_match_value_uuid.clone()} oninput={on_match_uuid} disabled={props.busy} class={classes!("w-full")} />
                        })}
                    </div>
                    <label class="flex items-center gap-3">
                        <input
                            type="checkbox"
                            class="checkbox"
                            checked={props.draft.policy_is_case_insensitive}
                            onchange={on_case}
                            disabled={props.busy}
                        />
                        <span>{"Case-insensitive matching"}</span>
                    </label>
                    {field("Rationale", "Optional operator rationale stored with the rule.", html! {
                        <Textarea value={props.draft.policy_rationale.clone()} rows={3} oninput={on_rationale} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Value set items", "Comma or newline separated text values used for in_set rules.", html! {
                        <Textarea value={props.draft.policy_value_set_text.clone()} rows={3} oninput={on_value_set} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <div class="flex flex-wrap gap-2">
                        <Button onclick={props.on_create_policy_set.clone()} disabled={props.busy}>{"Create policy set"}</Button>
                        <Button onclick={props.on_create_policy_rule.clone()} disabled={props.busy}>{"Create policy rule"}</Button>
                    </div>
                </div>
            </div>
        },
    )
}

#[function_component(ImportTorznabSection)]
fn import_torznab_section(props: &ImportTorznabSectionProps) -> Html {
    let on_profile_name = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_display_name = value;
    });
    let on_profile_id = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_public_id = value;
    });
    let on_profile_default_domain = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_default_media_domain_key = value;
    });
    let on_profile_domains = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_media_domain_keys = value;
    });
    let on_profile_indexers = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_indexer_public_ids = value;
    });
    let on_profile_allow_tags = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_tag_keys_allow = value;
    });
    let on_profile_block_tags = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_tag_keys_block = value;
    });
    let on_profile_prefer_tags = text_callback(props.draft.clone(), |draft, value| {
        draft.search_profile_tag_keys_prefer = value;
    });
    let on_import_source = text_callback(props.draft.clone(), |draft, value| {
        draft.import_job_source = value;
    });
    let on_import_format = text_callback(props.draft.clone(), |draft, value| {
        draft.import_job_payload_format = value;
    });
    let on_import_id = text_callback(props.draft.clone(), |draft, value| {
        draft.import_job_public_id = value;
    });
    let on_source_conflict_id = text_callback(props.draft.clone(), |draft, value| {
        draft.source_conflict_id = value;
    });
    let on_source_conflict_limit = text_callback(props.draft.clone(), |draft, value| {
        draft.source_conflict_limit = value;
    });
    let on_source_conflict_resolution = text_callback(props.draft.clone(), |draft, value| {
        draft.source_conflict_resolution = value;
    });
    let on_source_conflict_note = text_callback(props.draft.clone(), |draft, value| {
        draft.source_conflict_resolution_note = value;
    });
    let on_source_conflict_include_resolved = bool_callback(props.draft.clone(), |draft, value| {
        draft.source_conflict_include_resolved = value;
    });
    let on_prowlarr_url = text_callback(props.draft.clone(), |draft, value| {
        draft.prowlarr_base_url = value;
    });
    let on_prowlarr_key = text_callback(props.draft.clone(), |draft, value| {
        draft.prowlarr_api_key = value;
    });
    let on_dry_run = bool_callback(props.draft.clone(), |draft, value| {
        draft.import_dry_run = value
    });
    let on_backup = text_callback(props.draft.clone(), |draft, value| {
        draft.prowlarr_backup_payload = value;
    });
    let on_backup_snapshot = text_callback(props.draft.clone(), |draft, value| {
        draft.backup_snapshot_payload = value;
    });
    let on_torznab_profile = text_callback(props.draft.clone(), |draft, value| {
        draft.torznab_search_profile_public_id = value;
    });
    let on_torznab_name = text_callback(props.draft.clone(), |draft, value| {
        draft.torznab_display_name = value;
    });
    let on_torznab_id = text_callback(props.draft.clone(), |draft, value| {
        draft.torznab_instance_public_id = value;
    });
    let on_torznab_enabled = bool_callback(props.draft.clone(), |draft, value| {
        draft.torznab_is_enabled = value;
    });
    card(
        "Import jobs and Torznab",
        html! {
            <div class="grid gap-4 xl:grid-cols-2">
                <div class="space-y-4 rounded-box border border-base-300 p-4">
                    <h3 class="font-semibold">{"App sync"}</h3>
                    <p class="text-sm text-base-content/70">
                        {"Provision a downstream app path by reusing or creating a search profile, applying tag and indexer scoping, then issuing a Torznab endpoint for the app."}
                    </p>
                    <div class="grid gap-3 md:grid-cols-2">
                        {field("Search profile public ID", "Leave blank to create a new sync profile, or paste an existing one to reuse it.", html! {
                            <Input value={props.draft.search_profile_public_id.clone()} oninput={on_profile_id} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Search profile display name", "Used only when creating a new sync profile.", html! {
                            <Input value={props.draft.search_profile_display_name.clone()} oninput={on_profile_name} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Default media domain", "Optional per-app default domain such as movies or tv.", html! {
                            <Input value={props.draft.search_profile_default_media_domain_key.clone()} oninput={on_profile_default_domain} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Torznab display name", "Name exposed to the downstream Arr client.", html! {
                            <Input value={props.draft.torznab_display_name.clone()} oninput={on_torznab_name.clone()} disabled={props.busy} class={classes!("w-full")} />
                        })}
                    </div>
                    {field("Media domain allow-list", "Optional per-app category filter using ERD media-domain keys.", html! {
                        <Textarea value={props.draft.search_profile_media_domain_keys.clone()} rows={2} oninput={on_profile_domains} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Allowed indexer public IDs", "Optional explicit indexer allow-list for this app sync path.", html! {
                        <Textarea value={props.draft.search_profile_indexer_public_ids.clone()} rows={2} oninput={on_profile_indexers} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <div class="grid gap-3 md:grid-cols-3">
                        {field("Allow tags", "Tag keys that indexers must match for this app.", html! {
                            <Textarea value={props.draft.search_profile_tag_keys_allow.clone()} rows={2} oninput={on_profile_allow_tags} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Block tags", "Tag keys that should be excluded from this app.", html! {
                            <Textarea value={props.draft.search_profile_tag_keys_block.clone()} rows={2} oninput={on_profile_block_tags} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Prefer tags", "Tag keys preferred during ranking for this app.", html! {
                            <Textarea value={props.draft.search_profile_tag_keys_prefer.clone()} rows={2} oninput={on_profile_prefer_tags} disabled={props.busy} class={classes!("w-full")} />
                        })}
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button onclick={props.on_provision_app_sync.clone()} disabled={props.busy}>{"Provision app sync"}</Button>
                    </div>
                    <div class="space-y-3 border-t border-base-300 pt-3">
                        <h4 class="text-sm font-semibold">{"Provisioned app sync"}</h4>
                        {
                            if let Some(summary) = props.app_sync.summary.as_ref() {
                                render_app_sync_summary(summary)
                            } else {
                                html! {
                                    <p class="text-sm text-base-content/60">
                                        {"Provision a sync path to capture the search-profile UUID, Torznab UUID, and issued API key in one place."}
                                    </p>
                                }
                            }
                        }
                    </div>
                </div>
                <div class="space-y-4 rounded-box border border-base-300 p-4">
                    <h3 class="font-semibold">{"Import jobs"}</h3>
                    <div class="grid gap-3 md:grid-cols-2">
                        {field("Source system", "Use the seeded import source system key.", html! {
                            <Input value={props.draft.import_job_source.clone()} oninput={on_import_source} disabled={props.busy} class={classes!("w-full")} />
                        })}
                        {field("Payload format", "Use the seeded import payload format key.", html! {
                            <Input value={props.draft.import_job_payload_format.clone()} oninput={on_import_format} disabled={props.busy} class={classes!("w-full")} />
                        })}
                    </div>
                    {field("Import job public ID", "Use an existing import job to run or inspect.", html! {
                        <Input value={props.draft.import_job_public_id.clone()} oninput={on_import_id} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <label class="flex items-center gap-3">
                        <input
                            type="checkbox"
                            class="checkbox"
                            checked={props.draft.import_dry_run}
                            onchange={on_dry_run}
                            disabled={props.busy}
                        />
                        <span>{"Dry-run import execution"}</span>
                    </label>
                    {field("Prowlarr base URL", "Base URL for live import jobs.", html! {
                        <Input value={props.draft.prowlarr_base_url.clone()} oninput={on_prowlarr_url} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Prowlarr API key", "API key for live Prowlarr imports.", html! {
                        <Input value={props.draft.prowlarr_api_key.clone()} oninput={on_prowlarr_key} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Backup payload", "Paste the full Prowlarr backup JSON payload.", html! {
                        <Textarea value={props.draft.prowlarr_backup_payload.clone()} rows={6} oninput={on_backup} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <div class="flex flex-wrap gap-2">
                        <Button onclick={props.on_create_import_job.clone()} disabled={props.busy}>{"Create import job"}</Button>
                        <Button onclick={props.on_run_import_api.clone()} disabled={props.busy}>{"Run via API"}</Button>
                        <Button onclick={props.on_run_import_backup.clone()} disabled={props.busy}>{"Run via backup"}</Button>
                        <Button onclick={props.on_fetch_import_status.clone()} disabled={props.busy}>{"Fetch status"}</Button>
                        <Button onclick={props.on_fetch_import_results.clone()} disabled={props.busy}>{"Fetch results"}</Button>
                    </div>
                    <div class="space-y-3">
                        <h4 class="text-sm font-semibold">{"Import status"}</h4>
                        {
                            if let Some(status) = props.import_job.status.as_ref() {
                                render_import_status(status)
                            } else {
                                html! {
                                    <p class="text-sm text-base-content/60">
                                        {"Create or load an import job to keep status counts visible while you iterate on imports."}
                                    </p>
                                }
                            }
                        }
                    </div>
                    <div class="space-y-3">
                        <h4 class="text-sm font-semibold">{"Import results"}</h4>
                        {
                            if props.import_job.results.is_empty() {
                                html! {
                                    <p class="text-sm text-base-content/60">
                                        {"Fetch results to review unmapped definitions, duplicate skips, missing secrets, and imported instances."}
                                    </p>
                                }
                            } else {
                                html! {
                                    <div class="space-y-3">
                                        {for props.import_job.results.iter().map(render_import_result)}
                                    </div>
                                }
                            }
                        }
                    </div>
                    <div class="space-y-3 border-t border-base-300 pt-3">
                        <h4 class="text-sm font-semibold">{"Source conflict resolution"}</h4>
                        <p class="text-sm text-base-content/70">
                            {"Review durable metadata conflicts surfaced by ingest and apply the existing resolve/reopen procedures without leaving the import workflow."}
                        </p>
                        <div class="grid gap-3 md:grid-cols-2">
                            {field("Conflict ID", "Paste the numeric conflict ID returned by the conflict queue.", html! {
                                <Input value={props.draft.source_conflict_id.clone()} oninput={on_source_conflict_id} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Queue limit", "How many recent conflicts to load (1-200).", html! {
                                <Input value={props.draft.source_conflict_limit.clone()} oninput={on_source_conflict_limit} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Resolution", "Use an ERD conflict resolution key such as kept_existing or accepted_incoming.", html! {
                                <Input value={props.draft.source_conflict_resolution.clone()} oninput={on_source_conflict_resolution} disabled={props.busy} class={classes!("w-full")} />
                            })}
                            {field("Resolution note", "Optional audit note for resolve or reopen.", html! {
                                <Input value={props.draft.source_conflict_resolution_note.clone()} oninput={on_source_conflict_note} disabled={props.busy} class={classes!("w-full")} />
                            })}
                        </div>
                        <label class="flex items-center gap-3">
                            <input
                                type="checkbox"
                                class="checkbox"
                                checked={props.draft.source_conflict_include_resolved}
                                onchange={on_source_conflict_include_resolved}
                                disabled={props.busy}
                            />
                            <span>{"Include resolved conflicts in the queue"}</span>
                        </label>
                        <div class="flex flex-wrap gap-2">
                            <Button onclick={props.on_fetch_source_conflicts.clone()} disabled={props.busy}>{"Fetch conflicts"}</Button>
                            <Button onclick={props.on_resolve_source_conflict.clone()} disabled={props.busy}>{"Resolve conflict"}</Button>
                            <Button onclick={props.on_reopen_source_conflict.clone()} disabled={props.busy}>{"Reopen conflict"}</Button>
                        </div>
                        {
                            if props.source_conflicts.items.is_empty() {
                                html! {
                                    <p class="text-sm text-base-content/60">
                                        {"Fetch conflicts to review durable metadata mismatches before deciding whether to keep existing values or accept incoming ones."}
                                    </p>
                                }
                            } else {
                                html! {
                                    <div class="space-y-3">
                                        {for props.source_conflicts.items.iter().map(render_source_metadata_conflict)}
                                    </div>
                                }
                            }
                        }
                    </div>
                    <div class="space-y-3 border-t border-base-300 pt-3">
                        <h4 class="text-sm font-semibold">{"Backup & restore"}</h4>
                        <p class="text-sm text-base-content/70">
                            {"Export a sanitized snapshot of tags, rate limits, routing policies, indexer instances, and secret references. Secret values are never included."}
                        </p>
                        {field("Snapshot payload", "Review or paste the backup JSON document used for restore operations.", html! {
                            <Textarea
                                value={props.draft.backup_snapshot_payload.clone()}
                                rows={10}
                                oninput={on_backup_snapshot}
                                disabled={props.busy}
                                class={classes!("w-full", "font-mono", "text-xs")}
                            />
                        })}
                        <div class="flex flex-wrap gap-2">
                            <Button onclick={props.on_export_backup.clone()} disabled={props.busy}>{"Export backup"}</Button>
                            <Button onclick={props.on_restore_backup.clone()} disabled={props.busy}>{"Restore backup"}</Button>
                        </div>
                        {
                            if let Some(snapshot) = props.backup.snapshot.as_ref() {
                                render_backup_snapshot(snapshot)
                            } else {
                                html! {
                                    <p class="text-sm text-base-content/60">
                                        {"Export a snapshot to keep a restorable copy of the current indexer configuration graph."}
                                    </p>
                                }
                            }
                        }
                        {
                            if props.backup.unresolved_secret_bindings.is_empty() {
                                html! {}
                            } else {
                                html! {
                                    <div class="space-y-2">
                                        <h5 class="text-sm font-semibold">{"Unresolved secret bindings"}</h5>
                                        {for props.backup.unresolved_secret_bindings.iter().map(render_unresolved_secret_binding)}
                                    </div>
                                }
                            }
                        }
                    </div>
                </div>
                <div class="space-y-4 rounded-box border border-base-300 p-4 xl:col-span-2">
                    <h3 class="font-semibold">{"Torznab instances"}</h3>
                    {field("Search-profile public ID", "Required for Torznab instance creation.", html! {
                        <Input value={props.draft.torznab_search_profile_public_id.clone()} oninput={on_torznab_profile} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Display name", "Torznab display name returned to clients.", html! {
                        <Input value={props.draft.torznab_display_name.clone()} oninput={on_torznab_name} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    {field("Torznab instance public ID", "Use for key rotation, state changes, and deletion.", html! {
                        <Input value={props.draft.torznab_instance_public_id.clone()} oninput={on_torznab_id} disabled={props.busy} class={classes!("w-full")} />
                    })}
                    <label class="flex items-center gap-3">
                        <input
                            type="checkbox"
                            class="checkbox"
                            checked={props.draft.torznab_is_enabled}
                            onchange={on_torznab_enabled}
                            disabled={props.busy}
                        />
                        <span>{"Enable Torznab instance when setting state"}</span>
                    </label>
                    <div class="flex flex-wrap gap-2">
                        <Button onclick={props.on_create_torznab.clone()} disabled={props.busy}>{"Create Torznab instance"}</Button>
                        <Button onclick={props.on_rotate_torznab.clone()} disabled={props.busy}>{"Rotate API key"}</Button>
                        <Button onclick={props.on_set_torznab_state.clone()} disabled={props.busy}>{"Set state"}</Button>
                        <Button onclick={props.on_delete_torznab.clone()} disabled={props.busy}>{"Delete Torznab instance"}</Button>
                    </div>
                </div>
            </div>
        },
    )
}

#[function_component(ActivityLog)]
fn activity_log(props: &ActivityLogProps) -> Html {
    card(
        "Activity log",
        html! {
            <div class="space-y-3">
                {if props.records.is_empty() {
                    html! { <EmptyState title="No admin activity yet" body="Run an indexer action to capture the response payload here." /> }
                } else {
                    html! {
                        <div class="space-y-3">
                            {for props.records.iter().map(|record| html! {
                                <article class="rounded-box border border-base-300 bg-base-200/40 p-4">
                                    <h3 class="font-medium">{record.title.clone()}</h3>
                                    <pre class="mt-3 overflow-x-auto whitespace-pre-wrap text-xs">{record.body.clone()}</pre>
                                </article>
                            })}
                        </div>
                    }
                }}
            </div>
        },
    )
}
