//! Indexer application services backed by the data layer.
//!
//! # Design
//! - Map API-level indexer operations onto stored-procedure calls.
//! - Convert data-layer failures into stable, typed service errors.
//! - Preserve origin-only logging by translating propagated data errors without re-logging them.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::to_string as to_json_string;
use serde_yaml::Value as YamlValue;
use tracing::{Instrument, info_span};
use uuid::Uuid;

use revaer_api::app::indexers::{
    CategoryMappingServiceError, CategoryMappingServiceErrorKind,
    HealthNotificationHookUpdateParams, HealthNotificationServiceError,
    HealthNotificationServiceErrorKind, ImportJobServiceError, ImportJobServiceErrorKind,
    IndexerBackupServiceError, IndexerBackupServiceErrorKind, IndexerCfStateResetParams,
    IndexerDefinitionServiceError, IndexerDefinitionServiceErrorKind, IndexerFacade,
    IndexerHealthEventListParams, IndexerInstanceFieldError, IndexerInstanceFieldErrorKind,
    IndexerInstanceFieldValueParams, IndexerInstanceServiceError, IndexerInstanceServiceErrorKind,
    IndexerInstanceTestFinalizeParams, IndexerInstanceUpdateParams, IndexerRssSeenListParams,
    IndexerRssSeenMarkParams, IndexerRssSubscriptionParams, IndexerSourceReputationListParams,
    PolicyRuleCreateParams, PolicyServiceError, PolicyServiceErrorKind,
    RateLimitPolicyServiceError, RateLimitPolicyServiceErrorKind, RoutingPolicyServiceError,
    RoutingPolicyServiceErrorKind, SearchProfileServiceError, SearchProfileServiceErrorKind,
    SearchRequestCreateParams, SearchRequestServiceError, SearchRequestServiceErrorKind,
    SecretServiceError, SecretServiceErrorKind, SourceMetadataConflictServiceError,
    SourceMetadataConflictServiceErrorKind, TagServiceError, TagServiceErrorKind,
    TorznabAccessError, TorznabAccessErrorKind, TorznabCategory, TorznabInstanceAuth,
    TorznabInstanceCredentials, TorznabInstanceServiceError, TorznabInstanceServiceErrorKind,
    TrackerCategoryMappingDeleteParams, TrackerCategoryMappingUpsertParams,
};
use revaer_api::models::{
    CardigannDefinitionImportResponse, ImportJobResultResponse, ImportJobStatusResponse,
    IndexerBackupExportResponse, IndexerBackupFieldItem, IndexerBackupIndexerInstanceItem,
    IndexerBackupRateLimitPolicyItem, IndexerBackupRestoreResponse,
    IndexerBackupRoutingParameterItem, IndexerBackupRoutingPolicyItem, IndexerBackupSecretRef,
    IndexerBackupSnapshot, IndexerBackupTagItem, IndexerBackupUnresolvedSecretBinding,
    IndexerCfStateResponse, IndexerConnectivityProfileResponse, IndexerDefinitionResponse,
    IndexerHealthEventResponse, IndexerHealthNotificationHookResponse,
    IndexerInstanceFieldInventoryResponse, IndexerInstanceListItemResponse,
    IndexerInstanceTestFinalizeResponse, IndexerInstanceTestPrepareResponse,
    IndexerRssSeenItemResponse, IndexerRssSeenMarkResponse, IndexerRssSubscriptionResponse,
    IndexerSourceMetadataConflictResponse, IndexerSourceReputationResponse,
    PolicyRuleListItemResponse, PolicySetListItemResponse, RateLimitPolicyListItemResponse,
    RoutingPolicyDetailResponse, RoutingPolicyListItemResponse, RoutingPolicyParameterResponse,
    SearchPageItemResponse, SearchPageListResponse, SearchPageResponse, SearchPageSummaryResponse,
    SearchProfileListItemResponse, SearchRequestCreateResponse,
    SearchRequestExplainabilityResponse, SecretMetadataResponse, TagListItemResponse,
    TorznabInstanceListItemResponse,
};
use revaer_config::ConfigService;
use revaer_data::DataError;
use revaer_data::indexers::backup::{
    BackupIndexerInstanceRow, BackupRateLimitPolicyRow, BackupRoutingPolicyRow, BackupTagRow,
    indexer_backup_export_indexer_instance_list, indexer_backup_export_rate_limit_policy_list,
    indexer_backup_export_routing_policy_list, indexer_backup_export_tag_list,
};
use revaer_data::indexers::category_mapping::{
    TrackerCategoryMappingDeleteInput, TrackerCategoryMappingUpsertInput,
    media_domain_mapping_delete, media_domain_mapping_upsert, tracker_category_mapping_delete,
    tracker_category_mapping_resolve_feed, tracker_category_mapping_upsert,
};
use revaer_data::indexers::cf_state::{indexer_cf_state_get, indexer_cf_state_reset};
use revaer_data::indexers::conflicts::{
    SourceMetadataConflictRow, source_metadata_conflict_list, source_metadata_conflict_reopen,
    source_metadata_conflict_resolve,
};
use revaer_data::indexers::connectivity::{
    indexer_connectivity_profile_get, indexer_health_event_list, indexer_source_reputation_list,
};
use revaer_data::indexers::definitions::{
    CardigannDefinitionFieldImport, ImportedIndexerDefinitionRow, IndexerDefinitionRow,
    indexer_definition_import_cardigann_begin, indexer_definition_import_cardigann_complete,
    indexer_definition_import_cardigann_field, indexer_definition_list,
};
use revaer_data::indexers::executor::{
    IndexerTestFinalizeInput, indexer_instance_test_finalize, indexer_instance_test_prepare,
};
use revaer_data::indexers::import_jobs::{
    ImportJobResultRow, ImportJobStatusRow, import_job_create, import_job_get_status,
    import_job_list_results, import_job_run_prowlarr_api, import_job_run_prowlarr_backup,
};
use revaer_data::indexers::instances::{
    IndexerInstanceFieldValueInput, IndexerInstanceUpdateInput, indexer_instance_create,
    indexer_instance_field_bind_secret, indexer_instance_field_set_value,
    indexer_instance_set_media_domains, indexer_instance_set_tags, indexer_instance_update,
    rss_subscription_set,
};
use revaer_data::indexers::notifications::{
    IndexerHealthNotificationHookRow, IndexerHealthNotificationHookUpdateInput,
    indexer_health_notification_hook_create, indexer_health_notification_hook_delete,
    indexer_health_notification_hook_get, indexer_health_notification_hook_list,
    indexer_health_notification_hook_update,
};
use revaer_data::indexers::policies::{
    PolicyRuleCreateInput, PolicyRuleValueItem as DataPolicyRuleValueItem, PolicySetRuleListRow,
    policy_rule_create, policy_rule_disable, policy_rule_enable, policy_rule_reorder,
    policy_set_create, policy_set_disable, policy_set_enable, policy_set_reorder,
    policy_set_rule_list, policy_set_update,
};
use revaer_data::indexers::rate_limits::{
    indexer_instance_set_rate_limit_policy, rate_limit_policy_create,
    rate_limit_policy_soft_delete, rate_limit_policy_update, routing_policy_set_rate_limit_policy,
};
use revaer_data::indexers::routing::{
    routing_policy_bind_secret, routing_policy_create, routing_policy_get, routing_policy_set_param,
};
use revaer_data::indexers::rss::{
    RssSeenMarkInput, rss_item_seen_list, rss_item_seen_mark, rss_subscription_get,
};
use revaer_data::indexers::search_pages::{
    SearchPageFetchRow, SearchPageSummaryRow, SearchRequestExplainabilityRow, search_page_fetch,
    search_page_list, search_request_explainability,
};
use revaer_data::indexers::search_profiles::{
    SearchProfileListRow, search_profile_add_policy_set, search_profile_create,
    search_profile_indexer_allow, search_profile_indexer_block, search_profile_list,
    search_profile_remove_policy_set, search_profile_set_default,
    search_profile_set_default_domain, search_profile_set_domain_allowlist,
    search_profile_tag_allow, search_profile_tag_block, search_profile_tag_prefer,
    search_profile_update,
};
use revaer_data::indexers::search_requests::{
    SearchRequestCreateInput, search_request_cancel, search_request_create,
};
use revaer_data::indexers::secrets::{
    secret_create, secret_metadata_list, secret_revoke, secret_rotate,
};
use revaer_data::indexers::tags::{tag_create, tag_list, tag_soft_delete, tag_update};
use revaer_data::indexers::torznab::{
    TorznabInstanceListRow, torznab_category_list, torznab_download_prepare,
    torznab_instance_authenticate, torznab_instance_create, torznab_instance_enable_disable,
    torznab_instance_list, torznab_instance_rotate_key, torznab_instance_soft_delete,
};
use revaer_telemetry::Metrics;

pub(crate) struct IndexerService {
    config: Arc<ConfigService>,
    telemetry: Metrics,
}

impl IndexerService {
    pub(crate) const fn new(config: Arc<ConfigService>, telemetry: Metrics) -> Self {
        Self { config, telemetry }
    }

    async fn run_operation<T, E, DataErr, Future, Mapper>(
        &self,
        operation: &'static str,
        future: Future,
        map_error: Mapper,
    ) -> Result<T, E>
    where
        Future: std::future::Future<Output = Result<T, DataErr>>,
        Mapper: FnOnce(&DataErr) -> E,
    {
        let started = Instant::now();
        let result = future.await;
        let outcome = if result.is_ok() { "success" } else { "error" };
        self.telemetry.inc_indexer_operation(operation, outcome);
        self.telemetry
            .observe_indexer_operation_latency(operation, outcome, started.elapsed());
        result.map_err(|error| map_error(&error))
    }

    async fn run_data_operation<T, E, Future>(
        &self,
        operation: &'static str,
        error_operation: &'static str,
        future: Future,
        map_error: fn(&'static str, &DataError) -> E,
    ) -> Result<T, E>
    where
        Future: std::future::Future<Output = Result<T, DataError>>,
    {
        self.run_operation(operation, future, |error| map_error(error_operation, error))
            .await
    }

    async fn run_unlabeled_data_operation<T, E, Future>(
        &self,
        operation: &'static str,
        future: Future,
        map_error: fn(&DataError) -> E,
    ) -> Result<T, E>
    where
        Future: std::future::Future<Output = Result<T, DataError>>,
    {
        self.run_operation(operation, future, map_error).await
    }

    async fn restore_backup_tags(
        &self,
        actor_user_public_id: Uuid,
        tags: &[IndexerBackupTagItem],
    ) -> Result<i32, IndexerBackupServiceError> {
        let mut created_tag_count = 0_i32;
        for tag in tags {
            self.tag_create(actor_user_public_id, &tag.tag_key, &tag.display_name)
                .await
                .map_err(|error| map_indexer_backup_tag_error(&error))?;
            created_tag_count += 1;
        }
        Ok(created_tag_count)
    }

    async fn restore_backup_rate_limits(
        &self,
        actor_user_public_id: Uuid,
        policies: &[IndexerBackupRateLimitPolicyItem],
    ) -> Result<(i32, BTreeMap<String, Uuid>), IndexerBackupServiceError> {
        let existing_system_policy_ids = self
            .rate_limit_policy_list(actor_user_public_id)
            .await
            .map_err(|error| map_indexer_backup_rate_limit_error(&error))?
            .into_iter()
            .filter(|policy| policy.is_system)
            .map(|policy| (policy.display_name, policy.rate_limit_policy_public_id))
            .collect::<BTreeMap<_, _>>();
        let mut rate_limit_id_by_name = BTreeMap::new();
        let mut created_rate_limit_policy_count = 0_i32;
        for policy in policies {
            if policy.is_system {
                let policy_public_id = lookup_backup_reference(
                    &existing_system_policy_ids,
                    &policy.display_name,
                    "rate_limit_reference_missing",
                )?;
                rate_limit_id_by_name.insert(policy.display_name.clone(), policy_public_id);
                continue;
            }
            let policy_public_id = self
                .rate_limit_policy_create(
                    actor_user_public_id,
                    &policy.display_name,
                    policy.requests_per_minute,
                    policy.burst,
                    policy.concurrent_requests,
                )
                .await
                .map_err(|error| map_indexer_backup_rate_limit_error(&error))?;
            rate_limit_id_by_name.insert(policy.display_name.clone(), policy_public_id);
            created_rate_limit_policy_count += 1;
        }
        Ok((created_rate_limit_policy_count, rate_limit_id_by_name))
    }

    async fn restore_backup_routing_policies(
        &self,
        actor_user_public_id: Uuid,
        policies: &[IndexerBackupRoutingPolicyItem],
        rate_limit_id_by_name: &BTreeMap<String, Uuid>,
    ) -> Result<
        (
            i32,
            BTreeMap<String, Uuid>,
            Vec<IndexerBackupUnresolvedSecretBinding>,
        ),
        IndexerBackupServiceError,
    > {
        let mut unresolved_secret_bindings = Vec::new();
        let mut routing_policy_id_by_name = BTreeMap::new();
        let mut created_routing_policy_count = 0_i32;

        for policy in policies {
            let routing_policy_public_id = self
                .routing_policy_create(actor_user_public_id, &policy.display_name, &policy.mode)
                .await
                .map_err(|error| map_indexer_backup_routing_error(&error))?;
            routing_policy_id_by_name.insert(policy.display_name.clone(), routing_policy_public_id);
            if let Some(rate_limit_display_name) = policy.rate_limit_display_name.as_deref() {
                let rate_limit_policy_public_id = lookup_backup_reference(
                    rate_limit_id_by_name,
                    rate_limit_display_name,
                    "rate_limit_reference_missing",
                )?;
                self.routing_policy_set_rate_limit_policy(
                    actor_user_public_id,
                    routing_policy_public_id,
                    Some(rate_limit_policy_public_id),
                )
                .await
                .map_err(|error| map_indexer_backup_rate_limit_error(&error))?;
            }
            for parameter in &policy.parameters {
                self.restore_backup_routing_parameter(
                    actor_user_public_id,
                    &policy.display_name,
                    routing_policy_public_id,
                    parameter,
                    &mut unresolved_secret_bindings,
                )
                .await?;
            }
            created_routing_policy_count += 1;
        }

        Ok((
            created_routing_policy_count,
            routing_policy_id_by_name,
            unresolved_secret_bindings,
        ))
    }

    async fn restore_backup_routing_parameter(
        &self,
        actor_user_public_id: Uuid,
        policy_display_name: &str,
        routing_policy_public_id: Uuid,
        parameter: &IndexerBackupRoutingParameterItem,
        unresolved_secret_bindings: &mut Vec<IndexerBackupUnresolvedSecretBinding>,
    ) -> Result<(), IndexerBackupServiceError> {
        if parameter.value_plain.is_some()
            || parameter.value_int.is_some()
            || parameter.value_bool.is_some()
        {
            self.routing_policy_set_param(
                actor_user_public_id,
                routing_policy_public_id,
                &parameter.param_key,
                parameter.value_plain.as_deref(),
                parameter.value_int,
                parameter.value_bool,
            )
            .await
            .map_err(|error| map_indexer_backup_routing_error(&error))?;
        }
        if let Some(secret_public_id) = parameter.secret_public_id {
            match self
                .routing_policy_bind_secret(
                    actor_user_public_id,
                    routing_policy_public_id,
                    &parameter.param_key,
                    secret_public_id,
                )
                .await
            {
                Ok(()) => {}
                Err(error) if is_missing_secret_error(error.code()) => {
                    unresolved_secret_bindings.push(IndexerBackupUnresolvedSecretBinding {
                        entity_type: "routing_policy".to_string(),
                        entity_display_name: policy_display_name.to_string(),
                        binding_key: parameter.param_key.clone(),
                        secret_public_id,
                    });
                }
                Err(error) => return Err(map_indexer_backup_routing_error(&error)),
            }
        }
        Ok(())
    }

    async fn restore_backup_indexer_instances(
        &self,
        actor_user_public_id: Uuid,
        indexer_instances: &[IndexerBackupIndexerInstanceItem],
        rate_limit_id_by_name: &BTreeMap<String, Uuid>,
        routing_policy_id_by_name: &BTreeMap<String, Uuid>,
    ) -> Result<(i32, Vec<IndexerBackupUnresolvedSecretBinding>), IndexerBackupServiceError> {
        let mut unresolved_secret_bindings = Vec::new();
        let mut created_indexer_instance_count = 0_i32;

        for instance in indexer_instances {
            let routing_policy_public_id = instance
                .routing_policy_display_name
                .as_deref()
                .map(|display_name| {
                    lookup_backup_reference(
                        routing_policy_id_by_name,
                        display_name,
                        "routing_policy_reference_missing",
                    )
                })
                .transpose()?;
            let indexer_instance_public_id = self
                .indexer_instance_create(
                    actor_user_public_id,
                    &instance.upstream_slug,
                    &instance.display_name,
                    Some(instance.priority),
                    instance.trust_tier_key.as_deref(),
                    routing_policy_public_id,
                )
                .await
                .map_err(|error| map_indexer_backup_indexer_error(&error))?;
            self.indexer_instance_update(IndexerInstanceUpdateParams {
                actor_user_public_id,
                indexer_instance_public_id,
                display_name: None,
                priority: Some(instance.priority),
                trust_tier_key: instance.trust_tier_key.as_deref(),
                routing_policy_public_id,
                is_enabled: Some(instance.instance_status == "enabled"),
                enable_rss: Some(instance.rss_status == "enabled"),
                enable_automatic_search: Some(instance.automatic_search_status == "enabled"),
                enable_interactive_search: Some(instance.interactive_search_status == "enabled"),
            })
            .await
            .map_err(|error| map_indexer_backup_indexer_error(&error))?;

            self.restore_backup_indexer_instance_links(
                actor_user_public_id,
                indexer_instance_public_id,
                instance,
                rate_limit_id_by_name,
            )
            .await?;
            self.restore_backup_indexer_instance_fields(
                actor_user_public_id,
                indexer_instance_public_id,
                instance,
                &mut unresolved_secret_bindings,
            )
            .await?;
            if let Some(rss_subscription_enabled) = instance.rss_subscription_enabled {
                let _ = self
                    .indexer_rss_subscription_set(IndexerRssSubscriptionParams {
                        actor_user_public_id,
                        indexer_instance_public_id,
                        is_enabled: rss_subscription_enabled,
                        interval_seconds: instance.rss_interval_seconds,
                    })
                    .await
                    .map_err(|error| map_indexer_backup_indexer_error(&error))?;
            }
            created_indexer_instance_count += 1;
        }

        Ok((created_indexer_instance_count, unresolved_secret_bindings))
    }

    async fn restore_backup_indexer_instance_links(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
        instance: &IndexerBackupIndexerInstanceItem,
        rate_limit_id_by_name: &BTreeMap<String, Uuid>,
    ) -> Result<(), IndexerBackupServiceError> {
        if let Some(rate_limit_display_name) = instance.rate_limit_display_name.as_deref() {
            let rate_limit_policy_public_id = lookup_backup_reference(
                rate_limit_id_by_name,
                rate_limit_display_name,
                "rate_limit_reference_missing",
            )?;
            self.indexer_instance_set_rate_limit_policy(
                actor_user_public_id,
                indexer_instance_public_id,
                Some(rate_limit_policy_public_id),
            )
            .await
            .map_err(|error| map_indexer_backup_rate_limit_error(&error))?;
        }
        if !instance.media_domain_keys.is_empty() {
            self.indexer_instance_set_media_domains(
                actor_user_public_id,
                indexer_instance_public_id,
                &instance.media_domain_keys,
            )
            .await
            .map_err(|error| map_indexer_backup_indexer_error(&error))?;
        }
        if !instance.tag_keys.is_empty() {
            self.indexer_instance_set_tags(
                actor_user_public_id,
                indexer_instance_public_id,
                None,
                Some(&instance.tag_keys),
            )
            .await
            .map_err(|error| map_indexer_backup_indexer_error(&error))?;
        }
        Ok(())
    }

    async fn restore_backup_indexer_instance_fields(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
        instance: &IndexerBackupIndexerInstanceItem,
        unresolved_secret_bindings: &mut Vec<IndexerBackupUnresolvedSecretBinding>,
    ) -> Result<(), IndexerBackupServiceError> {
        for field in &instance.fields {
            if field.secret_public_id.is_none() {
                self.indexer_instance_field_set_value(IndexerInstanceFieldValueParams {
                    actor_user_public_id,
                    indexer_instance_public_id,
                    field_name: &field.field_name,
                    value_plain: field.value_plain.as_deref(),
                    value_int: field.value_int,
                    value_decimal: field.value_decimal.as_deref(),
                    value_bool: field.value_bool,
                })
                .await
                .map_err(|error| map_indexer_backup_field_error(&error))?;
            }
            if let Some(secret_public_id) = field.secret_public_id {
                match self
                    .indexer_instance_field_bind_secret(
                        actor_user_public_id,
                        indexer_instance_public_id,
                        &field.field_name,
                        secret_public_id,
                    )
                    .await
                {
                    Ok(()) => {}
                    Err(error) if is_missing_secret_error(error.code()) => {
                        unresolved_secret_bindings.push(IndexerBackupUnresolvedSecretBinding {
                            entity_type: "indexer_instance".to_string(),
                            entity_display_name: instance.display_name.clone(),
                            binding_key: field.field_name.clone(),
                            secret_public_id,
                        });
                    }
                    Err(error) => return Err(map_indexer_backup_field_error(&error)),
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct CardigannDefinitionDocument {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    caps: Option<YamlValue>,
    #[serde(default)]
    login: Option<YamlValue>,
    #[serde(default)]
    search: Option<YamlValue>,
    #[serde(default)]
    test: Option<YamlValue>,
    #[serde(default)]
    settings: Vec<CardigannSettingDocument>,
}

#[derive(Debug, Deserialize)]
struct CardigannSettingDocument {
    name: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(rename = "type")]
    setting_type: String,
    #[serde(default)]
    required: Option<bool>,
    #[serde(default)]
    advanced: Option<bool>,
    #[serde(default)]
    default: Option<YamlValue>,
    #[serde(default)]
    options: Vec<CardigannSettingOptionDocument>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CardigannSettingOptionDocument {
    Simple(String),
    Labeled {
        value: String,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        name: Option<String>,
    },
}

#[derive(Debug)]
struct PreparedCardigannDefinitionImport {
    upstream_slug: String,
    display_name: String,
    canonical_definition_text: String,
    fields: Vec<PreparedCardigannFieldImport>,
}

#[derive(Debug, Serialize)]
struct CanonicalCardigannDefinition {
    id: String,
    name: String,
    description: Option<String>,
    caps: Option<YamlValue>,
    login: Option<YamlValue>,
    search: Option<YamlValue>,
    test: Option<YamlValue>,
    settings: Vec<CanonicalCardigannSetting>,
}

#[derive(Debug, Serialize)]
struct CanonicalCardigannSetting {
    name: String,
    label: String,
    setting_type: String,
    required: bool,
    advanced: bool,
    default_value: Option<String>,
    options: Vec<CanonicalCardigannSettingOption>,
}

#[derive(Debug, Serialize)]
struct CanonicalCardigannSettingOption {
    value: String,
    label: String,
}

#[derive(Debug)]
struct PreparedCardigannFieldImport {
    field_name: String,
    label: String,
    field_type: String,
    is_required: bool,
    is_advanced: bool,
    display_order: i32,
    default_value_plain: Option<String>,
    default_value_int: Option<i32>,
    default_value_decimal: Option<String>,
    default_value_bool: Option<bool>,
    option_values: Vec<String>,
    option_labels: Vec<String>,
}

impl PreparedCardigannFieldImport {
    fn to_data_import(&self) -> CardigannDefinitionFieldImport<'_> {
        CardigannDefinitionFieldImport {
            field_name: &self.field_name,
            label: &self.label,
            field_type: &self.field_type,
            is_required: self.is_required,
            is_advanced: self.is_advanced,
            display_order: self.display_order,
            default_value_plain: self.default_value_plain.as_deref(),
            default_value_int: self.default_value_int,
            default_value_decimal: self.default_value_decimal.as_deref(),
            default_value_bool: self.default_value_bool,
            option_values: self.option_values.clone(),
            option_labels: self.option_labels.clone(),
        }
    }
}

#[async_trait]
impl IndexerFacade for IndexerService {
    async fn indexer_definition_list(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<Vec<IndexerDefinitionResponse>, IndexerDefinitionServiceError> {
        let span = info_span!(
            "indexer.definition_list",
            actor_user_public_id = %actor_user_public_id
        );
        let definitions = self
            .run_operation(
                "indexer.definition_list",
                indexer_definition_list(self.config.pool(), actor_user_public_id).instrument(span),
                |error| map_indexer_definition_error("indexer_definition_list", error),
            )
            .await?;

        Ok(definitions
            .into_iter()
            .map(map_indexer_definition_row)
            .collect())
    }

    async fn indexer_definition_import_cardigann(
        &self,
        actor_user_public_id: Uuid,
        yaml_payload: &str,
        is_deprecated: Option<bool>,
    ) -> Result<CardigannDefinitionImportResponse, IndexerDefinitionServiceError> {
        let prepared = parse_cardigann_definition_import(yaml_payload)?;
        let span = info_span!(
            "indexer.definition_import_cardigann",
            actor_user_public_id = %actor_user_public_id,
            upstream_slug = %prepared.upstream_slug
        );
        let summary = self
            .run_operation(
                "indexer.definition_import_cardigann",
                async {
                    let mut tx = self.config.pool().begin().await?;
                    let _definition = indexer_definition_import_cardigann_begin(
                        &mut tx,
                        actor_user_public_id,
                        &prepared.upstream_slug,
                        &prepared.display_name,
                        &prepared.canonical_definition_text,
                        is_deprecated.unwrap_or(false),
                    )
                    .await?;
                    for field in &prepared.fields {
                        let field_import = field.to_data_import();
                        indexer_definition_import_cardigann_field(
                            &mut tx,
                            actor_user_public_id,
                            &prepared.upstream_slug,
                            &field_import,
                        )
                        .await?;
                    }
                    let summary = indexer_definition_import_cardigann_complete(
                        &mut tx,
                        actor_user_public_id,
                        &prepared.upstream_slug,
                    )
                    .await?;
                    tx.commit().await.map_err(DataError::from)?;
                    Ok(summary)
                }
                .instrument(span),
                |error| map_indexer_definition_error("indexer_definition_import_cardigann", error),
            )
            .await?;

        Ok(map_imported_indexer_definition_row(summary))
    }

    async fn tag_create(
        &self,
        actor_user_public_id: Uuid,
        tag_key: &str,
        display_name: &str,
    ) -> Result<Uuid, TagServiceError> {
        let span = info_span!("indexer.tag_create", actor_user_public_id = %actor_user_public_id);
        self.run_data_operation(
            "indexer.tag_create",
            "tag_create",
            tag_create(
                self.config.pool(),
                actor_user_public_id,
                tag_key,
                display_name,
            )
            .instrument(span),
            map_tag_error,
        )
        .await
    }

    async fn tag_list(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<Vec<TagListItemResponse>, TagServiceError> {
        let span = info_span!("indexer.tag_list", actor_user_public_id = %actor_user_public_id);
        let rows = self
            .run_data_operation(
                "indexer.tag_list",
                "tag_list",
                tag_list(self.config.pool(), actor_user_public_id).instrument(span),
                map_tag_error,
            )
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| TagListItemResponse {
                tag_public_id: row.tag_public_id,
                tag_key: row.tag_key,
                display_name: row.display_name,
                updated_at: row.updated_at,
            })
            .collect())
    }

    async fn tag_update(
        &self,
        actor_user_public_id: Uuid,
        tag_public_id: Option<Uuid>,
        tag_key: Option<&str>,
        display_name: &str,
    ) -> Result<Uuid, TagServiceError> {
        let span = info_span!("indexer.tag_update", actor_user_public_id = %actor_user_public_id);
        self.run_data_operation(
            "indexer.tag_update",
            "tag_update",
            tag_update(
                self.config.pool(),
                actor_user_public_id,
                tag_public_id,
                tag_key,
                display_name,
            )
            .instrument(span),
            map_tag_error,
        )
        .await
    }

    async fn tag_delete(
        &self,
        actor_user_public_id: Uuid,
        tag_public_id: Option<Uuid>,
        tag_key: Option<&str>,
    ) -> Result<(), TagServiceError> {
        let span = info_span!("indexer.tag_delete", actor_user_public_id = %actor_user_public_id);
        self.run_data_operation(
            "indexer.tag_delete",
            "tag_delete",
            tag_soft_delete(
                self.config.pool(),
                actor_user_public_id,
                tag_public_id,
                tag_key,
            )
            .instrument(span),
            map_tag_error,
        )
        .await
    }

    async fn indexer_health_notification_hook_list(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<Vec<IndexerHealthNotificationHookResponse>, HealthNotificationServiceError> {
        let span = info_span!("indexer.health_notification_hook_list", actor_user_public_id = %actor_user_public_id);
        let rows = self
            .run_data_operation(
                "indexer.health_notification_hook_list",
                "indexer_health_notification_hook_list",
                indexer_health_notification_hook_list(self.config.pool(), actor_user_public_id)
                    .instrument(span),
                map_health_notification_error,
            )
            .await?;
        Ok(rows
            .into_iter()
            .map(map_health_notification_hook_row)
            .collect())
    }

    async fn indexer_health_notification_hook_create(
        &self,
        actor_user_public_id: Uuid,
        channel: &str,
        display_name: &str,
        status_threshold: &str,
        webhook_url: Option<&str>,
        email: Option<&str>,
    ) -> Result<Uuid, HealthNotificationServiceError> {
        let span = info_span!(
            "indexer.health_notification_hook_create",
            actor_user_public_id = %actor_user_public_id
        );
        self.run_data_operation(
            "indexer.health_notification_hook_create",
            "indexer_health_notification_hook_create",
            indexer_health_notification_hook_create(
                self.config.pool(),
                actor_user_public_id,
                channel,
                display_name,
                status_threshold,
                webhook_url,
                email,
            )
            .instrument(span),
            map_health_notification_error,
        )
        .await
    }

    async fn indexer_health_notification_hook_get(
        &self,
        actor_user_public_id: Uuid,
        hook_public_id: Uuid,
    ) -> Result<IndexerHealthNotificationHookResponse, HealthNotificationServiceError> {
        let span = info_span!("indexer.health_notification_hook_get", actor_user_public_id = %actor_user_public_id, hook_public_id = %hook_public_id);
        let row = self
            .run_data_operation(
                "indexer.health_notification_hook_get",
                "indexer_health_notification_hook_get",
                indexer_health_notification_hook_get(
                    self.config.pool(),
                    actor_user_public_id,
                    hook_public_id,
                )
                .instrument(span),
                map_health_notification_error,
            )
            .await?;
        Ok(map_health_notification_hook_row(row))
    }

    async fn indexer_health_notification_hook_update(
        &self,
        params: HealthNotificationHookUpdateParams<'_>,
    ) -> Result<Uuid, HealthNotificationServiceError> {
        let span = info_span!(
            "indexer.health_notification_hook_update",
            actor_user_public_id = %params.actor_user_public_id,
            hook_public_id = %params.hook_public_id
        );
        self.run_data_operation(
            "indexer.health_notification_hook_update",
            "indexer_health_notification_hook_update",
            indexer_health_notification_hook_update(
                self.config.pool(),
                &IndexerHealthNotificationHookUpdateInput {
                    actor_user_public_id: params.actor_user_public_id,
                    hook_public_id: params.hook_public_id,
                    display_name: params.display_name,
                    status_threshold: params.status_threshold,
                    webhook_url: params.webhook_url,
                    email: params.email,
                    is_enabled: params.is_enabled,
                },
            )
            .instrument(span),
            map_health_notification_error,
        )
        .await
    }

    async fn indexer_health_notification_hook_delete(
        &self,
        actor_user_public_id: Uuid,
        hook_public_id: Uuid,
    ) -> Result<(), HealthNotificationServiceError> {
        let span = info_span!(
            "indexer.health_notification_hook_delete",
            actor_user_public_id = %actor_user_public_id,
            hook_public_id = %hook_public_id
        );
        self.run_data_operation(
            "indexer.health_notification_hook_delete",
            "indexer_health_notification_hook_delete",
            indexer_health_notification_hook_delete(
                self.config.pool(),
                actor_user_public_id,
                hook_public_id,
            )
            .instrument(span),
            map_health_notification_error,
        )
        .await
    }

    async fn search_request_create(
        &self,
        params: SearchRequestCreateParams<'_>,
    ) -> Result<SearchRequestCreateResponse, SearchRequestServiceError> {
        let span = info_span!(
            "indexer.search_request_create",
            actor_user_public_id = %params.actor_user_public_id.unwrap_or_default()
        );
        let input = SearchRequestCreateInput {
            actor_user_public_id: params.actor_user_public_id,
            query_text: params.query_text,
            query_type: params.query_type,
            torznab_mode: params.torznab_mode,
            requested_media_domain_key: params.requested_media_domain_key,
            page_size: params.page_size,
            search_profile_public_id: params.search_profile_public_id,
            request_policy_set_public_id: params.request_policy_set_public_id,
            season_number: params.season_number,
            episode_number: params.episode_number,
            identifier_types: params.identifier_types,
            identifier_values: params.identifier_values,
            torznab_cat_ids: params.torznab_cat_ids,
        };

        let row = self
            .run_operation(
                "indexer.search_request_create",
                search_request_create(self.config.pool(), &input).instrument(span),
                |error| map_search_request_error("search_request_create", error),
            )
            .await?;

        Ok(SearchRequestCreateResponse {
            search_request_public_id: row.search_request_public_id,
            request_policy_set_public_id: row.request_policy_set_public_id,
        })
    }

    async fn search_request_cancel(
        &self,
        actor_user_public_id: Uuid,
        search_request_public_id: Uuid,
    ) -> Result<(), SearchRequestServiceError> {
        let span = info_span!(
            "indexer.search_request_cancel",
            actor_user_public_id = %actor_user_public_id
        );
        self.run_data_operation(
            "indexer.search_request_cancel",
            "search_request_cancel",
            search_request_cancel(
                self.config.pool(),
                actor_user_public_id,
                search_request_public_id,
            )
            .instrument(span),
            map_search_request_error,
        )
        .await
    }

    async fn search_page_list(
        &self,
        actor_user_public_id: Uuid,
        search_request_public_id: Uuid,
    ) -> Result<SearchPageListResponse, SearchRequestServiceError> {
        let span = info_span!(
            "indexer.search_page_list",
            actor_user_public_id = %actor_user_public_id,
            search_request_public_id = %search_request_public_id
        );
        let rows = self
            .run_operation(
                "indexer.search_page_list",
                search_page_list(
                    self.config.pool(),
                    actor_user_public_id,
                    search_request_public_id,
                )
                .instrument(span.clone()),
                |error| map_search_request_error("search_page_list", error),
            )
            .await?;
        let explainability = self
            .run_operation(
                "indexer.search_request_explainability",
                search_request_explainability(
                    self.config.pool(),
                    actor_user_public_id,
                    search_request_public_id,
                )
                .instrument(span),
                |error| map_search_request_error("search_request_explainability", error),
            )
            .await?;

        Ok(SearchPageListResponse {
            pages: rows.iter().map(map_search_page_summary).collect(),
            explainability: map_search_request_explainability(&explainability),
        })
    }

    async fn search_page_fetch(
        &self,
        actor_user_public_id: Uuid,
        search_request_public_id: Uuid,
        page_number: i32,
    ) -> Result<SearchPageResponse, SearchRequestServiceError> {
        let span = info_span!(
            "indexer.search_page_fetch",
            actor_user_public_id = %actor_user_public_id,
            search_request_public_id = %search_request_public_id,
            page_number = page_number
        );

        let rows = self
            .run_operation(
                "indexer.search_page_fetch",
                search_page_fetch(
                    self.config.pool(),
                    actor_user_public_id,
                    search_request_public_id,
                    page_number,
                )
                .instrument(span),
                |error| map_search_request_error("search_page_fetch", error),
            )
            .await?;

        let Some(first) = rows.first() else {
            return Err(
                SearchRequestServiceError::new(SearchRequestServiceErrorKind::NotFound)
                    .with_code("search_page_not_found"),
            );
        };

        let mut items = Vec::new();
        for row in &rows {
            if let Some(item) = map_search_page_item(row) {
                items.push(item);
            }
        }

        Ok(SearchPageResponse {
            page_number: first.page_number,
            sealed_at: first.sealed_at,
            item_count: first.item_count,
            items,
        })
    }

    async fn routing_policy_create(
        &self,
        actor_user_public_id: Uuid,
        display_name: &str,
        mode: &str,
    ) -> Result<Uuid, RoutingPolicyServiceError> {
        let span = info_span!(
            "indexer.routing_policy_create",
            actor_user_public_id = %actor_user_public_id
        );
        self.run_data_operation(
            "indexer.routing_policy_create",
            "routing_policy_create",
            routing_policy_create(self.config.pool(), actor_user_public_id, display_name, mode)
                .instrument(span),
            map_routing_policy_error,
        )
        .await
    }

    async fn routing_policy_set_param(
        &self,
        actor_user_public_id: Uuid,
        routing_policy_public_id: Uuid,
        param_key: &str,
        value_plain: Option<&str>,
        value_int: Option<i32>,
        value_bool: Option<bool>,
    ) -> Result<(), RoutingPolicyServiceError> {
        let span = info_span!(
            "indexer.routing_policy_set_param",
            actor_user_public_id = %actor_user_public_id,
            routing_policy_public_id = %routing_policy_public_id
        );
        self.run_data_operation(
            "indexer.routing_policy_set_param",
            "routing_policy_set_param",
            routing_policy_set_param(
                self.config.pool(),
                actor_user_public_id,
                routing_policy_public_id,
                param_key,
                value_plain,
                value_int,
                value_bool,
            )
            .instrument(span),
            map_routing_policy_error,
        )
        .await
    }

    async fn routing_policy_bind_secret(
        &self,
        actor_user_public_id: Uuid,
        routing_policy_public_id: Uuid,
        param_key: &str,
        secret_public_id: Uuid,
    ) -> Result<(), RoutingPolicyServiceError> {
        let span = info_span!(
            "indexer.routing_policy_bind_secret",
            actor_user_public_id = %actor_user_public_id,
            routing_policy_public_id = %routing_policy_public_id,
            secret_public_id = %secret_public_id
        );
        self.run_data_operation(
            "indexer.routing_policy_bind_secret",
            "routing_policy_bind_secret",
            routing_policy_bind_secret(
                self.config.pool(),
                actor_user_public_id,
                routing_policy_public_id,
                param_key,
                secret_public_id,
            )
            .instrument(span),
            map_routing_policy_error,
        )
        .await
    }

    async fn routing_policy_get(
        &self,
        actor_user_public_id: Uuid,
        routing_policy_public_id: Uuid,
    ) -> Result<RoutingPolicyDetailResponse, RoutingPolicyServiceError> {
        let span = info_span!(
            "indexer.routing_policy_get",
            actor_user_public_id = %actor_user_public_id,
            routing_policy_public_id = %routing_policy_public_id
        );
        let rows = self
            .run_operation(
                "indexer.routing_policy_get",
                routing_policy_get(
                    self.config.pool(),
                    actor_user_public_id,
                    routing_policy_public_id,
                )
                .instrument(span),
                |error| map_routing_policy_error("routing_policy_get", error),
            )
            .await?;

        let Some(first) = rows.first() else {
            return Err(
                RoutingPolicyServiceError::new(RoutingPolicyServiceErrorKind::NotFound)
                    .with_code("routing_policy_not_found"),
            );
        };

        let mut parameters = Vec::new();
        for row in &rows {
            if let Some(param_key) = row.param_key.clone() {
                parameters.push(RoutingPolicyParameterResponse {
                    param_key,
                    value_plain: row.value_plain.clone(),
                    value_int: row.value_int,
                    value_bool: row.value_bool,
                    secret_public_id: row.secret_public_id,
                    secret_binding_name: row.secret_binding_name.clone(),
                });
            }
        }

        Ok(RoutingPolicyDetailResponse {
            routing_policy_public_id: first.routing_policy_public_id,
            display_name: first.display_name.clone(),
            mode: first.mode.clone(),
            rate_limit_policy_public_id: first.rate_limit_policy_public_id,
            rate_limit_display_name: first.rate_limit_display_name.clone(),
            rate_limit_requests_per_minute: first.rate_limit_requests_per_minute,
            rate_limit_burst: first.rate_limit_burst,
            rate_limit_concurrent_requests: first.rate_limit_concurrent_requests,
            parameters,
        })
    }

    async fn routing_policy_list(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<Vec<RoutingPolicyListItemResponse>, RoutingPolicyServiceError> {
        let span = info_span!(
            "indexer.routing_policy_list",
            actor_user_public_id = %actor_user_public_id
        );
        let rows = self
            .run_operation(
                "indexer.routing_policy_list",
                indexer_backup_export_routing_policy_list(self.config.pool(), actor_user_public_id)
                    .instrument(span),
                |error| {
                    map_routing_policy_error("indexer_backup_export_routing_policy_list", error)
                },
            )
            .await?;

        Ok(build_routing_policy_inventory(&rows))
    }

    async fn rate_limit_policy_create(
        &self,
        actor_user_public_id: Uuid,
        display_name: &str,
        rpm: i32,
        burst: i32,
        concurrent: i32,
    ) -> Result<Uuid, RateLimitPolicyServiceError> {
        let span = info_span!(
            "indexer.rate_limit_policy_create",
            actor_user_public_id = %actor_user_public_id
        );
        self.run_data_operation(
            "indexer.rate_limit_policy_create",
            "rate_limit_policy_create",
            rate_limit_policy_create(
                self.config.pool(),
                actor_user_public_id,
                display_name,
                rpm,
                burst,
                concurrent,
            )
            .instrument(span),
            map_rate_limit_policy_error,
        )
        .await
    }

    async fn rate_limit_policy_update(
        &self,
        actor_user_public_id: Uuid,
        rate_limit_policy_public_id: Uuid,
        display_name: Option<&str>,
        rpm: Option<i32>,
        burst: Option<i32>,
        concurrent: Option<i32>,
    ) -> Result<(), RateLimitPolicyServiceError> {
        let span = info_span!(
            "indexer.rate_limit_policy_update",
            actor_user_public_id = %actor_user_public_id,
            rate_limit_policy_public_id = %rate_limit_policy_public_id
        );
        self.run_data_operation(
            "indexer.rate_limit_policy_update",
            "rate_limit_policy_update",
            rate_limit_policy_update(
                self.config.pool(),
                actor_user_public_id,
                rate_limit_policy_public_id,
                display_name,
                rpm,
                burst,
                concurrent,
            )
            .instrument(span),
            map_rate_limit_policy_error,
        )
        .await
    }

    async fn rate_limit_policy_soft_delete(
        &self,
        actor_user_public_id: Uuid,
        rate_limit_policy_public_id: Uuid,
    ) -> Result<(), RateLimitPolicyServiceError> {
        let span = info_span!(
            "indexer.rate_limit_policy_soft_delete",
            actor_user_public_id = %actor_user_public_id,
            rate_limit_policy_public_id = %rate_limit_policy_public_id
        );
        self.run_data_operation(
            "indexer.rate_limit_policy_soft_delete",
            "rate_limit_policy_soft_delete",
            rate_limit_policy_soft_delete(
                self.config.pool(),
                actor_user_public_id,
                rate_limit_policy_public_id,
            )
            .instrument(span),
            map_rate_limit_policy_error,
        )
        .await
    }

    async fn indexer_instance_set_rate_limit_policy(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
        rate_limit_policy_public_id: Option<Uuid>,
    ) -> Result<(), RateLimitPolicyServiceError> {
        let span = info_span!(
            "indexer.instance_set_rate_limit_policy",
            actor_user_public_id = %actor_user_public_id,
            indexer_instance_public_id = %indexer_instance_public_id,
            rate_limit_policy_public_id = ?rate_limit_policy_public_id
        );
        self.run_data_operation(
            "indexer.instance_set_rate_limit_policy",
            "indexer_instance_set_rate_limit_policy",
            indexer_instance_set_rate_limit_policy(
                self.config.pool(),
                actor_user_public_id,
                indexer_instance_public_id,
                rate_limit_policy_public_id,
            )
            .instrument(span),
            map_rate_limit_policy_error,
        )
        .await
    }

    async fn routing_policy_set_rate_limit_policy(
        &self,
        actor_user_public_id: Uuid,
        routing_policy_public_id: Uuid,
        rate_limit_policy_public_id: Option<Uuid>,
    ) -> Result<(), RateLimitPolicyServiceError> {
        let span = info_span!(
            "indexer.routing_policy_set_rate_limit_policy",
            actor_user_public_id = %actor_user_public_id,
            routing_policy_public_id = %routing_policy_public_id,
            rate_limit_policy_public_id = ?rate_limit_policy_public_id
        );
        self.run_data_operation(
            "indexer.routing_policy_set_rate_limit_policy",
            "routing_policy_set_rate_limit_policy",
            routing_policy_set_rate_limit_policy(
                self.config.pool(),
                actor_user_public_id,
                routing_policy_public_id,
                rate_limit_policy_public_id,
            )
            .instrument(span),
            map_rate_limit_policy_error,
        )
        .await
    }

    async fn rate_limit_policy_list(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<Vec<RateLimitPolicyListItemResponse>, RateLimitPolicyServiceError> {
        let span = info_span!(
            "indexer.rate_limit_policy_list",
            actor_user_public_id = %actor_user_public_id
        );
        let rows = self
            .run_operation(
                "indexer.rate_limit_policy_list",
                indexer_backup_export_rate_limit_policy_list(
                    self.config.pool(),
                    actor_user_public_id,
                )
                .instrument(span),
                |error| {
                    map_rate_limit_policy_error(
                        "indexer_backup_export_rate_limit_policy_list",
                        error,
                    )
                },
            )
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| RateLimitPolicyListItemResponse {
                rate_limit_policy_public_id: row.rate_limit_policy_public_id,
                display_name: row.display_name,
                requests_per_minute: row.requests_per_minute,
                burst: row.burst,
                concurrent_requests: row.concurrent_requests,
                is_system: row.is_system,
            })
            .collect())
    }

    async fn search_profile_create(
        &self,
        actor_user_public_id: Uuid,
        display_name: &str,
        is_default: Option<bool>,
        page_size: Option<i32>,
        default_media_domain_key: Option<&str>,
        user_public_id: Option<Uuid>,
    ) -> Result<Uuid, SearchProfileServiceError> {
        let span = info_span!(
            "indexer.search_profile_create",
            actor_user_public_id = %actor_user_public_id
        );
        self.run_data_operation(
            "indexer.search_profile_create",
            "search_profile_create",
            search_profile_create(
                self.config.pool(),
                actor_user_public_id,
                display_name,
                is_default,
                page_size,
                default_media_domain_key,
                user_public_id,
            )
            .instrument(span),
            map_search_profile_error,
        )
        .await
    }

    async fn search_profile_update(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        display_name: Option<&str>,
        page_size: Option<i32>,
    ) -> Result<Uuid, SearchProfileServiceError> {
        let span = info_span!(
            "indexer.search_profile_update",
            actor_user_public_id = %actor_user_public_id,
            search_profile_public_id = %search_profile_public_id
        );
        self.run_data_operation(
            "indexer.search_profile_update",
            "search_profile_update",
            search_profile_update(
                self.config.pool(),
                actor_user_public_id,
                search_profile_public_id,
                display_name,
                page_size,
            )
            .instrument(span),
            map_search_profile_error,
        )
        .await
    }

    async fn search_profile_set_default(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        page_size: Option<i32>,
    ) -> Result<(), SearchProfileServiceError> {
        let span = info_span!(
            "indexer.search_profile_set_default",
            actor_user_public_id = %actor_user_public_id,
            search_profile_public_id = %search_profile_public_id
        );
        self.run_data_operation(
            "indexer.search_profile_set_default",
            "search_profile_set_default",
            search_profile_set_default(
                self.config.pool(),
                actor_user_public_id,
                search_profile_public_id,
                page_size,
            )
            .instrument(span),
            map_search_profile_error,
        )
        .await
    }

    async fn search_profile_set_default_domain(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        default_media_domain_key: Option<&str>,
    ) -> Result<(), SearchProfileServiceError> {
        let span = info_span!(
            "indexer.search_profile_set_default_domain",
            actor_user_public_id = %actor_user_public_id,
            search_profile_public_id = %search_profile_public_id
        );
        self.run_data_operation(
            "indexer.search_profile_set_default_domain",
            "search_profile_set_default_domain",
            search_profile_set_default_domain(
                self.config.pool(),
                actor_user_public_id,
                search_profile_public_id,
                default_media_domain_key,
            )
            .instrument(span),
            map_search_profile_error,
        )
        .await
    }

    async fn search_profile_set_domain_allowlist(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        media_domain_keys: &[String],
    ) -> Result<(), SearchProfileServiceError> {
        let span = info_span!(
            "indexer.search_profile_set_domain_allowlist",
            actor_user_public_id = %actor_user_public_id,
            search_profile_public_id = %search_profile_public_id
        );
        self.run_data_operation(
            "indexer.search_profile_set_domain_allowlist",
            "search_profile_set_domain_allowlist",
            search_profile_set_domain_allowlist(
                self.config.pool(),
                actor_user_public_id,
                search_profile_public_id,
                media_domain_keys,
            )
            .instrument(span),
            map_search_profile_error,
        )
        .await
    }

    async fn search_profile_add_policy_set(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        policy_set_public_id: Uuid,
    ) -> Result<(), SearchProfileServiceError> {
        let span = info_span!(
            "indexer.search_profile_add_policy_set",
            actor_user_public_id = %actor_user_public_id,
            search_profile_public_id = %search_profile_public_id,
            policy_set_public_id = %policy_set_public_id
        );
        self.run_data_operation(
            "indexer.search_profile_add_policy_set",
            "search_profile_add_policy_set",
            search_profile_add_policy_set(
                self.config.pool(),
                actor_user_public_id,
                search_profile_public_id,
                policy_set_public_id,
            )
            .instrument(span),
            map_search_profile_error,
        )
        .await
    }

    async fn search_profile_remove_policy_set(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        policy_set_public_id: Uuid,
    ) -> Result<(), SearchProfileServiceError> {
        let span = info_span!(
            "indexer.search_profile_remove_policy_set",
            actor_user_public_id = %actor_user_public_id,
            search_profile_public_id = %search_profile_public_id,
            policy_set_public_id = %policy_set_public_id
        );
        self.run_data_operation(
            "indexer.search_profile_remove_policy_set",
            "search_profile_remove_policy_set",
            search_profile_remove_policy_set(
                self.config.pool(),
                actor_user_public_id,
                search_profile_public_id,
                policy_set_public_id,
            )
            .instrument(span),
            map_search_profile_error,
        )
        .await
    }

    async fn search_profile_indexer_allow(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        indexer_instance_public_ids: &[Uuid],
    ) -> Result<(), SearchProfileServiceError> {
        let span = info_span!(
            "indexer.search_profile_indexer_allow",
            actor_user_public_id = %actor_user_public_id,
            search_profile_public_id = %search_profile_public_id
        );
        self.run_data_operation(
            "indexer.search_profile_indexer_allow",
            "search_profile_indexer_allow",
            search_profile_indexer_allow(
                self.config.pool(),
                actor_user_public_id,
                search_profile_public_id,
                indexer_instance_public_ids,
            )
            .instrument(span),
            map_search_profile_error,
        )
        .await
    }

    async fn search_profile_indexer_block(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        indexer_instance_public_ids: &[Uuid],
    ) -> Result<(), SearchProfileServiceError> {
        let span = info_span!(
            "indexer.search_profile_indexer_block",
            actor_user_public_id = %actor_user_public_id,
            search_profile_public_id = %search_profile_public_id
        );
        self.run_data_operation(
            "indexer.search_profile_indexer_block",
            "search_profile_indexer_block",
            search_profile_indexer_block(
                self.config.pool(),
                actor_user_public_id,
                search_profile_public_id,
                indexer_instance_public_ids,
            )
            .instrument(span),
            map_search_profile_error,
        )
        .await
    }

    async fn search_profile_tag_allow(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        tag_public_ids: Option<&[Uuid]>,
        tag_keys: Option<&[String]>,
    ) -> Result<(), SearchProfileServiceError> {
        let span = info_span!(
            "indexer.search_profile_tag_allow",
            actor_user_public_id = %actor_user_public_id,
            search_profile_public_id = %search_profile_public_id
        );
        self.run_data_operation(
            "indexer.search_profile_tag_allow",
            "search_profile_tag_allow",
            search_profile_tag_allow(
                self.config.pool(),
                actor_user_public_id,
                search_profile_public_id,
                tag_public_ids,
                tag_keys,
            )
            .instrument(span),
            map_search_profile_error,
        )
        .await
    }

    async fn search_profile_tag_block(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        tag_public_ids: Option<&[Uuid]>,
        tag_keys: Option<&[String]>,
    ) -> Result<(), SearchProfileServiceError> {
        let span = info_span!(
            "indexer.search_profile_tag_block",
            actor_user_public_id = %actor_user_public_id,
            search_profile_public_id = %search_profile_public_id
        );
        self.run_data_operation(
            "indexer.search_profile_tag_block",
            "search_profile_tag_block",
            search_profile_tag_block(
                self.config.pool(),
                actor_user_public_id,
                search_profile_public_id,
                tag_public_ids,
                tag_keys,
            )
            .instrument(span),
            map_search_profile_error,
        )
        .await
    }

    async fn search_profile_tag_prefer(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        tag_public_ids: Option<&[Uuid]>,
        tag_keys: Option<&[String]>,
    ) -> Result<(), SearchProfileServiceError> {
        let span = info_span!(
            "indexer.search_profile_tag_prefer",
            actor_user_public_id = %actor_user_public_id,
            search_profile_public_id = %search_profile_public_id
        );
        self.run_data_operation(
            "indexer.search_profile_tag_prefer",
            "search_profile_tag_prefer",
            search_profile_tag_prefer(
                self.config.pool(),
                actor_user_public_id,
                search_profile_public_id,
                tag_public_ids,
                tag_keys,
            )
            .instrument(span),
            map_search_profile_error,
        )
        .await
    }

    async fn search_profile_list(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<Vec<SearchProfileListItemResponse>, SearchProfileServiceError> {
        let span = info_span!(
            "indexer.search_profile_list",
            actor_user_public_id = %actor_user_public_id
        );
        let rows = self
            .run_operation(
                "indexer.search_profile_list",
                search_profile_list(self.config.pool(), actor_user_public_id).instrument(span),
                |error| map_search_profile_error("search_profile_list", error),
            )
            .await?;

        Ok(rows
            .into_iter()
            .map(build_search_profile_inventory_item)
            .collect())
    }

    async fn import_job_create(
        &self,
        actor_user_public_id: Uuid,
        source: &str,
        is_dry_run: Option<bool>,
        target_search_profile_public_id: Option<Uuid>,
        target_torznab_instance_public_id: Option<Uuid>,
    ) -> Result<Uuid, ImportJobServiceError> {
        let span =
            info_span!("indexer.import_job_create", actor_user_public_id = %actor_user_public_id);
        self.run_data_operation(
            "indexer.import_job_create",
            "import_job_create",
            import_job_create(
                self.config.pool(),
                actor_user_public_id,
                source,
                is_dry_run,
                target_search_profile_public_id,
                target_torznab_instance_public_id,
            )
            .instrument(span),
            map_import_job_error,
        )
        .await
    }

    async fn import_job_run_prowlarr_api(
        &self,
        import_job_public_id: Uuid,
        prowlarr_url: &str,
        prowlarr_api_key_secret_public_id: Uuid,
    ) -> Result<(), ImportJobServiceError> {
        let span = info_span!(
            "indexer.import_job_run_prowlarr_api",
            import_job_public_id = %import_job_public_id
        );
        self.run_data_operation(
            "indexer.import_job_run_prowlarr_api",
            "import_job_run_prowlarr_api",
            import_job_run_prowlarr_api(
                self.config.pool(),
                import_job_public_id,
                prowlarr_url,
                prowlarr_api_key_secret_public_id,
            )
            .instrument(span),
            map_import_job_error,
        )
        .await
    }

    async fn import_job_run_prowlarr_backup(
        &self,
        import_job_public_id: Uuid,
        backup_blob_ref: &str,
    ) -> Result<(), ImportJobServiceError> {
        let span = info_span!(
            "indexer.import_job_run_prowlarr_backup",
            import_job_public_id = %import_job_public_id
        );
        self.run_data_operation(
            "indexer.import_job_run_prowlarr_backup",
            "import_job_run_prowlarr_backup",
            import_job_run_prowlarr_backup(
                self.config.pool(),
                import_job_public_id,
                backup_blob_ref,
            )
            .instrument(span),
            map_import_job_error,
        )
        .await
    }

    async fn import_job_get_status(
        &self,
        import_job_public_id: Uuid,
    ) -> Result<ImportJobStatusResponse, ImportJobServiceError> {
        let span = info_span!(
            "indexer.import_job_get_status",
            import_job_public_id = %import_job_public_id
        );
        let status = self
            .run_operation(
                "indexer.import_job_get_status",
                import_job_get_status(self.config.pool(), import_job_public_id).instrument(span),
                |error| map_import_job_error("import_job_get_status", error),
            )
            .await?;
        Ok(map_import_job_status(status))
    }

    async fn import_job_list_results(
        &self,
        import_job_public_id: Uuid,
    ) -> Result<Vec<ImportJobResultResponse>, ImportJobServiceError> {
        let span = info_span!(
            "indexer.import_job_list_results",
            import_job_public_id = %import_job_public_id
        );
        let results = self
            .run_operation(
                "indexer.import_job_list_results",
                import_job_list_results(self.config.pool(), import_job_public_id).instrument(span),
                |error| map_import_job_error("import_job_list_results", error),
            )
            .await?;
        Ok(results.into_iter().map(map_import_job_result).collect())
    }

    async fn source_metadata_conflict_list(
        &self,
        actor_user_public_id: Uuid,
        include_resolved: Option<bool>,
        limit: Option<i32>,
    ) -> Result<Vec<IndexerSourceMetadataConflictResponse>, SourceMetadataConflictServiceError>
    {
        let span = info_span!(
            "indexer.source_metadata_conflict_list",
            actor_user_public_id = %actor_user_public_id
        );
        let conflicts = self
            .run_operation(
                "indexer.source_metadata_conflict_list",
                source_metadata_conflict_list(
                    self.config.pool(),
                    actor_user_public_id,
                    include_resolved,
                    limit,
                )
                .instrument(span),
                map_source_metadata_conflict_error,
            )
            .await?;
        Ok(conflicts
            .into_iter()
            .map(map_source_metadata_conflict)
            .collect())
    }

    async fn source_metadata_conflict_resolve(
        &self,
        actor_user_public_id: Uuid,
        conflict_id: i64,
        resolution: &str,
        resolution_note: Option<&str>,
    ) -> Result<(), SourceMetadataConflictServiceError> {
        let span = info_span!(
            "indexer.source_metadata_conflict_resolve",
            actor_user_public_id = %actor_user_public_id,
            conflict_id = conflict_id
        );
        self.run_unlabeled_data_operation(
            "indexer.source_metadata_conflict_resolve",
            source_metadata_conflict_resolve(
                self.config.pool(),
                actor_user_public_id,
                conflict_id,
                resolution,
                resolution_note,
            )
            .instrument(span),
            map_source_metadata_conflict_error,
        )
        .await
    }

    async fn source_metadata_conflict_reopen(
        &self,
        actor_user_public_id: Uuid,
        conflict_id: i64,
        resolution_note: Option<&str>,
    ) -> Result<(), SourceMetadataConflictServiceError> {
        let span = info_span!(
            "indexer.source_metadata_conflict_reopen",
            actor_user_public_id = %actor_user_public_id,
            conflict_id = conflict_id
        );
        self.run_unlabeled_data_operation(
            "indexer.source_metadata_conflict_reopen",
            source_metadata_conflict_reopen(
                self.config.pool(),
                actor_user_public_id,
                conflict_id,
                resolution_note,
            )
            .instrument(span),
            map_source_metadata_conflict_error,
        )
        .await
    }

    async fn indexer_backup_export(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<IndexerBackupExportResponse, IndexerBackupServiceError> {
        let tags = self
            .run_operation(
                "indexer.backup_export.tags",
                indexer_backup_export_tag_list(self.config.pool(), actor_user_public_id),
                |error| map_indexer_backup_error("indexer_backup_export_tag_list", error),
            )
            .await?;
        let rate_limits = self
            .run_operation(
                "indexer.backup_export.rate_limits",
                indexer_backup_export_rate_limit_policy_list(
                    self.config.pool(),
                    actor_user_public_id,
                ),
                |error| {
                    map_indexer_backup_error("indexer_backup_export_rate_limit_policy_list", error)
                },
            )
            .await?;
        let routing_policies = self
            .run_operation(
                "indexer.backup_export.routing",
                indexer_backup_export_routing_policy_list(self.config.pool(), actor_user_public_id),
                |error| {
                    map_indexer_backup_error("indexer_backup_export_routing_policy_list", error)
                },
            )
            .await?;
        let indexer_instances = self
            .run_operation(
                "indexer.backup_export.instances",
                indexer_backup_export_indexer_instance_list(
                    self.config.pool(),
                    actor_user_public_id,
                ),
                |error| {
                    map_indexer_backup_error("indexer_backup_export_indexer_instance_list", error)
                },
            )
            .await?;

        Ok(IndexerBackupExportResponse {
            snapshot: build_backup_snapshot(
                tags,
                rate_limits,
                &routing_policies,
                &indexer_instances,
            ),
        })
    }

    async fn indexer_backup_restore(
        &self,
        actor_user_public_id: Uuid,
        snapshot: &IndexerBackupSnapshot,
    ) -> Result<IndexerBackupRestoreResponse, IndexerBackupServiceError> {
        let created_tag_count = self
            .restore_backup_tags(actor_user_public_id, &snapshot.tags)
            .await?;
        let (created_rate_limit_policy_count, rate_limit_id_by_name) = self
            .restore_backup_rate_limits(actor_user_public_id, &snapshot.rate_limit_policies)
            .await?;
        let (
            created_routing_policy_count,
            routing_policy_id_by_name,
            mut unresolved_secret_bindings,
        ) = self
            .restore_backup_routing_policies(
                actor_user_public_id,
                &snapshot.routing_policies,
                &rate_limit_id_by_name,
            )
            .await?;
        let (created_indexer_instance_count, instance_unresolved_secret_bindings) = self
            .restore_backup_indexer_instances(
                actor_user_public_id,
                &snapshot.indexer_instances,
                &rate_limit_id_by_name,
                &routing_policy_id_by_name,
            )
            .await?;
        unresolved_secret_bindings.extend(instance_unresolved_secret_bindings);

        Ok(IndexerBackupRestoreResponse {
            created_tag_count,
            created_rate_limit_policy_count,
            created_routing_policy_count,
            created_indexer_instance_count,
            unresolved_secret_bindings,
        })
    }

    async fn policy_set_create(
        &self,
        actor_user_public_id: Uuid,
        display_name: &str,
        scope: &str,
        enabled: Option<bool>,
    ) -> Result<Uuid, PolicyServiceError> {
        let span =
            info_span!("indexer.policy_set_create", actor_user_public_id = %actor_user_public_id);
        self.run_data_operation(
            "indexer.policy_set_create",
            "policy_set_create",
            policy_set_create(
                self.config.pool(),
                actor_user_public_id,
                display_name,
                scope,
                enabled,
            )
            .instrument(span),
            map_policy_error,
        )
        .await
    }

    async fn policy_set_update(
        &self,
        actor_user_public_id: Uuid,
        policy_set_public_id: Uuid,
        display_name: Option<&str>,
    ) -> Result<Uuid, PolicyServiceError> {
        let span = info_span!(
            "indexer.policy_set_update",
            actor_user_public_id = %actor_user_public_id,
            policy_set_public_id = %policy_set_public_id
        );
        self.run_data_operation(
            "indexer.policy_set_update",
            "policy_set_update",
            policy_set_update(
                self.config.pool(),
                actor_user_public_id,
                policy_set_public_id,
                display_name,
            )
            .instrument(span),
            map_policy_error,
        )
        .await
    }

    async fn policy_set_enable(
        &self,
        actor_user_public_id: Uuid,
        policy_set_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        let span = info_span!(
            "indexer.policy_set_enable",
            actor_user_public_id = %actor_user_public_id,
            policy_set_public_id = %policy_set_public_id
        );
        self.run_data_operation(
            "indexer.policy_set_enable",
            "policy_set_enable",
            policy_set_enable(
                self.config.pool(),
                actor_user_public_id,
                policy_set_public_id,
            )
            .instrument(span),
            map_policy_error,
        )
        .await
    }

    async fn policy_set_disable(
        &self,
        actor_user_public_id: Uuid,
        policy_set_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        let span = info_span!(
            "indexer.policy_set_disable",
            actor_user_public_id = %actor_user_public_id,
            policy_set_public_id = %policy_set_public_id
        );
        self.run_data_operation(
            "indexer.policy_set_disable",
            "policy_set_disable",
            policy_set_disable(
                self.config.pool(),
                actor_user_public_id,
                policy_set_public_id,
            )
            .instrument(span),
            map_policy_error,
        )
        .await
    }

    async fn policy_set_reorder(
        &self,
        actor_user_public_id: Uuid,
        ordered_policy_set_public_ids: &[Uuid],
    ) -> Result<(), PolicyServiceError> {
        let span = info_span!(
            "indexer.policy_set_reorder",
            actor_user_public_id = %actor_user_public_id
        );
        self.run_data_operation(
            "indexer.policy_set_reorder",
            "policy_set_reorder",
            policy_set_reorder(
                self.config.pool(),
                actor_user_public_id,
                ordered_policy_set_public_ids,
            )
            .instrument(span),
            map_policy_error,
        )
        .await
    }

    async fn policy_rule_create(
        &self,
        params: PolicyRuleCreateParams,
    ) -> Result<Uuid, PolicyServiceError> {
        let span = info_span!(
            "indexer.policy_rule_create",
            actor_user_public_id = %params.actor_user_public_id,
            policy_set_public_id = %params.policy_set_public_id
        );
        let value_set_items = params.value_set_items.as_ref().map(|items| {
            items
                .iter()
                .map(|item| DataPolicyRuleValueItem {
                    value_text: item.value_text.clone(),
                    value_int: item.value_int,
                    value_bigint: item.value_bigint,
                    value_uuid: item.value_uuid,
                })
                .collect::<Vec<_>>()
        });
        let input = PolicyRuleCreateInput {
            actor_user_public_id: params.actor_user_public_id,
            policy_set_public_id: params.policy_set_public_id,
            rule_type: &params.rule_type,
            match_field: &params.match_field,
            match_operator: &params.match_operator,
            sort_order: Some(params.sort_order),
            match_value_text: params.match_value_text.as_deref(),
            match_value_int: params.match_value_int,
            match_value_uuid: params.match_value_uuid,
            value_set_items: value_set_items.as_deref(),
            action: &params.action,
            severity: &params.severity,
            is_case_insensitive: params.is_case_insensitive,
            rationale: params.rationale.as_deref(),
            expires_at: params.expires_at,
        };
        self.run_data_operation(
            "indexer.policy_rule_create",
            "policy_rule_create",
            policy_rule_create(self.config.pool(), &input).instrument(span),
            map_policy_error,
        )
        .await
    }

    async fn policy_rule_enable(
        &self,
        actor_user_public_id: Uuid,
        policy_rule_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        let span = info_span!(
            "indexer.policy_rule_enable",
            actor_user_public_id = %actor_user_public_id,
            policy_rule_public_id = %policy_rule_public_id
        );
        self.run_data_operation(
            "indexer.policy_rule_enable",
            "policy_rule_enable",
            policy_rule_enable(
                self.config.pool(),
                actor_user_public_id,
                policy_rule_public_id,
            )
            .instrument(span),
            map_policy_error,
        )
        .await
    }

    async fn policy_rule_disable(
        &self,
        actor_user_public_id: Uuid,
        policy_rule_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        let span = info_span!(
            "indexer.policy_rule_disable",
            actor_user_public_id = %actor_user_public_id,
            policy_rule_public_id = %policy_rule_public_id
        );
        self.run_data_operation(
            "indexer.policy_rule_disable",
            "policy_rule_disable",
            policy_rule_disable(
                self.config.pool(),
                actor_user_public_id,
                policy_rule_public_id,
            )
            .instrument(span),
            map_policy_error,
        )
        .await
    }

    async fn policy_rule_reorder(
        &self,
        actor_user_public_id: Uuid,
        policy_set_public_id: Uuid,
        ordered_policy_rule_public_ids: &[Uuid],
    ) -> Result<(), PolicyServiceError> {
        let span = info_span!(
            "indexer.policy_rule_reorder",
            actor_user_public_id = %actor_user_public_id,
            policy_set_public_id = %policy_set_public_id
        );
        self.run_data_operation(
            "indexer.policy_rule_reorder",
            "policy_rule_reorder",
            policy_rule_reorder(
                self.config.pool(),
                actor_user_public_id,
                policy_set_public_id,
                ordered_policy_rule_public_ids,
            )
            .instrument(span),
            map_policy_error,
        )
        .await
    }

    async fn policy_set_list(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<Vec<PolicySetListItemResponse>, PolicyServiceError> {
        let span = info_span!(
            "indexer.policy_set_list",
            actor_user_public_id = %actor_user_public_id
        );
        let rows = self
            .run_operation(
                "indexer.policy_set_list",
                policy_set_rule_list(self.config.pool(), actor_user_public_id).instrument(span),
                |error| map_policy_error("policy_set_rule_list", error),
            )
            .await?;

        Ok(build_policy_set_inventory(&rows))
    }

    async fn tracker_category_mapping_upsert(
        &self,
        params: TrackerCategoryMappingUpsertParams<'_>,
    ) -> Result<(), CategoryMappingServiceError> {
        let span = info_span!(
            "indexer.tracker_category_mapping_upsert",
            actor_user_public_id = %params.actor_user_public_id
        );
        self.run_data_operation(
            "indexer.tracker_category_mapping_upsert",
            "tracker_category_mapping_upsert",
            tracker_category_mapping_upsert(
                self.config.pool(),
                params.actor_user_public_id,
                TrackerCategoryMappingUpsertInput {
                    torznab_instance_public_id: params.torznab_instance_public_id,
                    indexer_definition_upstream_slug: params.indexer_definition_upstream_slug,
                    indexer_instance_public_id: params.indexer_instance_public_id,
                    tracker_category: params.tracker_category,
                    tracker_subcategory: params.tracker_subcategory,
                    torznab_cat_id: params.torznab_cat_id,
                    media_domain_key: params.media_domain_key,
                },
            )
            .instrument(span),
            map_category_mapping_error,
        )
        .await
    }

    async fn tracker_category_mapping_delete(
        &self,
        params: TrackerCategoryMappingDeleteParams<'_>,
    ) -> Result<(), CategoryMappingServiceError> {
        let span = info_span!(
            "indexer.tracker_category_mapping_delete",
            actor_user_public_id = %params.actor_user_public_id
        );
        self.run_data_operation(
            "indexer.tracker_category_mapping_delete",
            "tracker_category_mapping_delete",
            tracker_category_mapping_delete(
                self.config.pool(),
                params.actor_user_public_id,
                TrackerCategoryMappingDeleteInput {
                    torznab_instance_public_id: params.torznab_instance_public_id,
                    indexer_definition_upstream_slug: params.indexer_definition_upstream_slug,
                    indexer_instance_public_id: params.indexer_instance_public_id,
                    tracker_category: params.tracker_category,
                    tracker_subcategory: params.tracker_subcategory,
                },
            )
            .instrument(span),
            map_category_mapping_error,
        )
        .await
    }

    async fn media_domain_mapping_upsert(
        &self,
        actor_user_public_id: Uuid,
        media_domain_key: &str,
        torznab_cat_id: i32,
        is_primary: Option<bool>,
    ) -> Result<(), CategoryMappingServiceError> {
        let span = info_span!(
            "indexer.media_domain_mapping_upsert",
            actor_user_public_id = %actor_user_public_id
        );
        self.run_data_operation(
            "indexer.media_domain_mapping_upsert",
            "media_domain_mapping_upsert",
            media_domain_mapping_upsert(
                self.config.pool(),
                actor_user_public_id,
                media_domain_key,
                torznab_cat_id,
                is_primary,
            )
            .instrument(span),
            map_category_mapping_error,
        )
        .await
    }

    async fn media_domain_mapping_delete(
        &self,
        actor_user_public_id: Uuid,
        media_domain_key: &str,
        torznab_cat_id: i32,
    ) -> Result<(), CategoryMappingServiceError> {
        let span = info_span!(
            "indexer.media_domain_mapping_delete",
            actor_user_public_id = %actor_user_public_id
        );
        self.run_data_operation(
            "indexer.media_domain_mapping_delete",
            "media_domain_mapping_delete",
            media_domain_mapping_delete(
                self.config.pool(),
                actor_user_public_id,
                media_domain_key,
                torznab_cat_id,
            )
            .instrument(span),
            map_category_mapping_error,
        )
        .await
    }

    async fn torznab_instance_create(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        display_name: &str,
    ) -> Result<TorznabInstanceCredentials, TorznabInstanceServiceError> {
        let span = info_span!(
            "indexer.torznab_instance_create",
            actor_user_public_id = %actor_user_public_id,
            search_profile_public_id = %search_profile_public_id
        );
        let value = self
            .run_data_operation(
                "indexer.torznab_instance_create",
                "torznab_instance_create",
                torznab_instance_create(
                    self.config.pool(),
                    actor_user_public_id,
                    Some(search_profile_public_id),
                    display_name,
                )
                .instrument(span),
                map_torznab_instance_error,
            )
            .await?;
        Ok(TorznabInstanceCredentials {
            torznab_instance_public_id: value.torznab_instance_public_id,
            api_key_plaintext: value.api_key_plaintext,
        })
    }

    async fn torznab_instance_rotate_key(
        &self,
        actor_user_public_id: Uuid,
        torznab_instance_public_id: Uuid,
    ) -> Result<TorznabInstanceCredentials, TorznabInstanceServiceError> {
        let span = info_span!(
            "indexer.torznab_instance_rotate_key",
            actor_user_public_id = %actor_user_public_id,
            torznab_instance_public_id = %torznab_instance_public_id
        );
        let value = self
            .run_data_operation(
                "indexer.torznab_instance_rotate_key",
                "torznab_instance_rotate_key",
                torznab_instance_rotate_key(
                    self.config.pool(),
                    actor_user_public_id,
                    torznab_instance_public_id,
                )
                .instrument(span),
                map_torznab_instance_error,
            )
            .await?;
        Ok(TorznabInstanceCredentials {
            torznab_instance_public_id,
            api_key_plaintext: value,
        })
    }

    async fn torznab_instance_enable_disable(
        &self,
        actor_user_public_id: Uuid,
        torznab_instance_public_id: Uuid,
        is_enabled: bool,
    ) -> Result<(), TorznabInstanceServiceError> {
        let span = info_span!(
            "indexer.torznab_instance_enable_disable",
            actor_user_public_id = %actor_user_public_id,
            torznab_instance_public_id = %torznab_instance_public_id,
            is_enabled = is_enabled
        );
        self.run_data_operation(
            "indexer.torznab_instance_enable_disable",
            "torznab_instance_enable_disable",
            torznab_instance_enable_disable(
                self.config.pool(),
                actor_user_public_id,
                torznab_instance_public_id,
                is_enabled,
            )
            .instrument(span),
            map_torznab_instance_error,
        )
        .await
    }

    async fn torznab_instance_soft_delete(
        &self,
        actor_user_public_id: Uuid,
        torznab_instance_public_id: Uuid,
    ) -> Result<(), TorznabInstanceServiceError> {
        let span = info_span!(
            "indexer.torznab_instance_soft_delete",
            actor_user_public_id = %actor_user_public_id,
            torznab_instance_public_id = %torznab_instance_public_id
        );
        self.run_data_operation(
            "indexer.torznab_instance_soft_delete",
            "torznab_instance_soft_delete",
            torznab_instance_soft_delete(
                self.config.pool(),
                actor_user_public_id,
                torznab_instance_public_id,
            )
            .instrument(span),
            map_torznab_instance_error,
        )
        .await
    }

    async fn torznab_instance_list(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<Vec<TorznabInstanceListItemResponse>, TorznabInstanceServiceError> {
        let span = info_span!(
            "indexer.torznab_instance_list",
            actor_user_public_id = %actor_user_public_id
        );
        let rows = self
            .run_data_operation(
                "indexer.torznab_instance_list",
                "torznab_instance_list",
                torznab_instance_list(self.config.pool(), actor_user_public_id).instrument(span),
                map_torznab_instance_error,
            )
            .await?;
        Ok(rows
            .into_iter()
            .map(build_torznab_instance_inventory_item)
            .collect())
    }

    async fn torznab_instance_authenticate(
        &self,
        torznab_instance_public_id: Uuid,
        api_key_plaintext: &str,
    ) -> Result<TorznabInstanceAuth, TorznabAccessError> {
        let span = info_span!(
            "torznab.instance_authenticate",
            torznab_instance_public_id = %torznab_instance_public_id
        );
        let row = self
            .run_data_operation(
                "indexer.torznab_instance_authenticate",
                "torznab_instance_authenticate",
                torznab_instance_authenticate(
                    self.config.pool(),
                    torznab_instance_public_id,
                    api_key_plaintext,
                )
                .instrument(span),
                map_torznab_access_error,
            )
            .await?;

        Ok(TorznabInstanceAuth {
            torznab_instance_id: row.torznab_instance_id,
            search_profile_id: row.search_profile_id,
            display_name: row.display_name,
        })
    }

    async fn torznab_download_prepare(
        &self,
        torznab_instance_public_id: Uuid,
        canonical_torrent_source_public_id: Uuid,
    ) -> Result<Option<String>, TorznabAccessError> {
        let span = info_span!(
            "torznab.download_prepare",
            torznab_instance_public_id = %torznab_instance_public_id,
            canonical_torrent_source_public_id = %canonical_torrent_source_public_id
        );
        self.run_data_operation(
            "indexer.torznab_download_prepare",
            "torznab_download_prepare",
            torznab_download_prepare(
                self.config.pool(),
                torznab_instance_public_id,
                canonical_torrent_source_public_id,
            )
            .instrument(span),
            map_torznab_access_error,
        )
        .await
    }

    async fn torznab_category_list(&self) -> Result<Vec<TorznabCategory>, TorznabAccessError> {
        let span = info_span!("torznab.category_list");
        let rows = self
            .run_data_operation(
                "indexer.torznab_category_list",
                "torznab_category_list",
                torznab_category_list(self.config.pool()).instrument(span),
                map_torznab_access_error,
            )
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| TorznabCategory {
                torznab_cat_id: row.torznab_cat_id,
                name: row.name,
            })
            .collect())
    }

    async fn torznab_feed_category_ids(
        &self,
        torznab_instance_public_id: Uuid,
        indexer_instance_public_id: Uuid,
        tracker_category: Option<i32>,
        tracker_subcategory: Option<i32>,
    ) -> Result<Vec<i32>, TorznabAccessError> {
        let span = info_span!(
            "torznab.feed_category_ids",
            torznab_instance_public_id = %torznab_instance_public_id,
            indexer_instance_public_id = %indexer_instance_public_id
        );
        self.run_data_operation(
            "indexer.torznab_feed_category_ids",
            "torznab_feed_category_ids",
            tracker_category_mapping_resolve_feed(
                self.config.pool(),
                torznab_instance_public_id,
                indexer_instance_public_id,
                tracker_category,
                tracker_subcategory,
            )
            .instrument(span),
            map_torznab_access_error,
        )
        .await
    }

    async fn indexer_instance_create(
        &self,
        actor_user_public_id: Uuid,
        indexer_definition_upstream_slug: &str,
        display_name: &str,
        priority: Option<i32>,
        trust_tier_key: Option<&str>,
        routing_policy_public_id: Option<Uuid>,
    ) -> Result<Uuid, IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.instance_create",
            actor_user_public_id = %actor_user_public_id,
            indexer_definition_upstream_slug = indexer_definition_upstream_slug,
            routing_policy_public_id = ?routing_policy_public_id
        );
        self.run_data_operation(
            "indexer.instance_create",
            "indexer_instance_create",
            indexer_instance_create(
                self.config.pool(),
                actor_user_public_id,
                indexer_definition_upstream_slug,
                display_name,
                priority,
                trust_tier_key,
                routing_policy_public_id,
            )
            .instrument(span),
            map_indexer_instance_error,
        )
        .await
    }

    async fn indexer_instance_update(
        &self,
        params: IndexerInstanceUpdateParams<'_>,
    ) -> Result<Uuid, IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.instance_update",
            actor_user_public_id = %params.actor_user_public_id,
            indexer_instance_public_id = %params.indexer_instance_public_id
        );
        self.run_data_operation(
            "indexer.instance_update",
            "indexer_instance_update",
            indexer_instance_update(
                self.config.pool(),
                &IndexerInstanceUpdateInput {
                    actor_user_public_id: params.actor_user_public_id,
                    indexer_instance_public_id: params.indexer_instance_public_id,
                    display_name: params.display_name,
                    priority: params.priority,
                    trust_tier_key: params.trust_tier_key,
                    routing_policy_public_id: params.routing_policy_public_id,
                    is_enabled: params.is_enabled,
                    enable_rss: params.enable_rss,
                    enable_automatic_search: params.enable_automatic_search,
                    enable_interactive_search: params.enable_interactive_search,
                },
            )
            .instrument(span),
            map_indexer_instance_error,
        )
        .await
    }

    async fn indexer_instance_list(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<Vec<IndexerInstanceListItemResponse>, IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.instance_list",
            actor_user_public_id = %actor_user_public_id
        );
        let rows = self
            .run_operation(
                "indexer.instance_list",
                indexer_backup_export_indexer_instance_list(
                    self.config.pool(),
                    actor_user_public_id,
                )
                .instrument(span),
                |error| {
                    map_indexer_instance_error("indexer_backup_export_indexer_instance_list", error)
                },
            )
            .await?;

        Ok(build_indexer_instance_inventory(&rows))
    }

    async fn indexer_instance_set_media_domains(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
        media_domain_keys: &[String],
    ) -> Result<(), IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.instance_set_media_domains",
            actor_user_public_id = %actor_user_public_id,
            indexer_instance_public_id = %indexer_instance_public_id
        );
        self.run_data_operation(
            "indexer.instance_set_media_domains",
            "indexer_instance_set_media_domains",
            indexer_instance_set_media_domains(
                self.config.pool(),
                actor_user_public_id,
                indexer_instance_public_id,
                media_domain_keys,
            )
            .instrument(span),
            map_indexer_instance_error,
        )
        .await
    }

    async fn indexer_instance_set_tags(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
        tag_public_ids: Option<&[Uuid]>,
        tag_keys: Option<&[String]>,
    ) -> Result<(), IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.instance_set_tags",
            actor_user_public_id = %actor_user_public_id,
            indexer_instance_public_id = %indexer_instance_public_id
        );
        self.run_data_operation(
            "indexer.instance_set_tags",
            "indexer_instance_set_tags",
            indexer_instance_set_tags(
                self.config.pool(),
                actor_user_public_id,
                indexer_instance_public_id,
                tag_public_ids,
                tag_keys,
            )
            .instrument(span),
            map_indexer_instance_error,
        )
        .await
    }

    async fn indexer_instance_field_set_value(
        &self,
        params: IndexerInstanceFieldValueParams<'_>,
    ) -> Result<(), IndexerInstanceFieldError> {
        let span = info_span!(
            "indexer.instance_field_set_value",
            actor_user_public_id = %params.actor_user_public_id,
            indexer_instance_public_id = %params.indexer_instance_public_id,
            field_name = params.field_name
        );
        let input = IndexerInstanceFieldValueInput {
            actor_user_public_id: params.actor_user_public_id,
            indexer_instance_public_id: params.indexer_instance_public_id,
            field_name: params.field_name,
            value_plain: params.value_plain,
            value_int: params.value_int,
            value_decimal: params.value_decimal,
            value_bool: params.value_bool,
        };
        self.run_data_operation(
            "indexer.instance_field_set_value",
            "indexer_instance_field_set_value",
            indexer_instance_field_set_value(self.config.pool(), &input).instrument(span),
            map_indexer_instance_field_error,
        )
        .await
    }

    async fn indexer_instance_field_bind_secret(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
        field_name: &str,
        secret_public_id: Uuid,
    ) -> Result<(), IndexerInstanceFieldError> {
        let span = info_span!(
            "indexer.instance_field_bind_secret",
            actor_user_public_id = %actor_user_public_id,
            indexer_instance_public_id = %indexer_instance_public_id,
            secret_public_id = %secret_public_id,
            field_name = field_name
        );
        self.run_data_operation(
            "indexer.instance_field_bind_secret",
            "indexer_instance_field_bind_secret",
            indexer_instance_field_bind_secret(
                self.config.pool(),
                actor_user_public_id,
                indexer_instance_public_id,
                field_name,
                secret_public_id,
            )
            .instrument(span),
            map_indexer_instance_field_error,
        )
        .await
    }

    async fn indexer_cf_state_reset(
        &self,
        params: IndexerCfStateResetParams<'_>,
    ) -> Result<(), IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.cf_state_reset",
            actor_user_public_id = %params.actor_user_public_id,
            indexer_instance_public_id = %params.indexer_instance_public_id
        );
        self.run_data_operation(
            "indexer.cf_state_reset",
            "indexer_cf_state_reset",
            indexer_cf_state_reset(
                self.config.pool(),
                params.actor_user_public_id,
                params.indexer_instance_public_id,
                params.reason,
            )
            .instrument(span),
            map_indexer_instance_error,
        )
        .await
    }

    async fn indexer_cf_state_get(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
    ) -> Result<IndexerCfStateResponse, IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.cf_state_get",
            actor_user_public_id = %actor_user_public_id,
            indexer_instance_public_id = %indexer_instance_public_id
        );
        let row = self
            .run_data_operation(
                "indexer.cf_state_get",
                "indexer_cf_state_get",
                indexer_cf_state_get(
                    self.config.pool(),
                    actor_user_public_id,
                    indexer_instance_public_id,
                )
                .instrument(span),
                map_indexer_instance_error,
            )
            .await?;

        Ok(IndexerCfStateResponse {
            state: row.state,
            last_changed_at: row.last_changed_at,
            cf_session_expires_at: row.cf_session_expires_at,
            cooldown_until: row.cooldown_until,
            backoff_seconds: row.backoff_seconds,
            consecutive_failures: row.consecutive_failures,
            last_error_class: row.last_error_class,
        })
    }

    async fn indexer_connectivity_profile_get(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
    ) -> Result<IndexerConnectivityProfileResponse, IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.connectivity_profile_get",
            actor_user_public_id = %actor_user_public_id,
            indexer_instance_public_id = %indexer_instance_public_id
        );
        let row = self
            .run_data_operation(
                "indexer.connectivity_profile_get",
                "indexer_connectivity_profile_get",
                indexer_connectivity_profile_get(
                    self.config.pool(),
                    actor_user_public_id,
                    indexer_instance_public_id,
                )
                .instrument(span),
                map_indexer_instance_error,
            )
            .await?;

        Ok(IndexerConnectivityProfileResponse {
            profile_exists: row.profile_exists,
            status: row.status,
            error_class: row.error_class,
            latency_p50_ms: row.latency_p50_ms,
            latency_p95_ms: row.latency_p95_ms,
            success_rate_1h: row.success_rate_1h,
            success_rate_24h: row.success_rate_24h,
            last_checked_at: row.last_checked_at,
        })
    }

    async fn indexer_source_reputation_list(
        &self,
        params: IndexerSourceReputationListParams<'_>,
    ) -> Result<Vec<IndexerSourceReputationResponse>, IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.source_reputation_list",
            actor_user_public_id = %params.actor_user_public_id,
            indexer_instance_public_id = %params.indexer_instance_public_id,
            window_key = params.window_key.unwrap_or("1h")
        );
        let rows = self
            .run_data_operation(
                "indexer.source_reputation_list",
                "indexer_source_reputation_list",
                indexer_source_reputation_list(
                    self.config.pool(),
                    params.actor_user_public_id,
                    params.indexer_instance_public_id,
                    params.window_key,
                    params.limit,
                )
                .instrument(span),
                map_indexer_instance_error,
            )
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| IndexerSourceReputationResponse {
                window_key: row.window_key,
                window_start: row.window_start,
                request_success_rate: row.request_success_rate,
                acquisition_success_rate: row.acquisition_success_rate,
                fake_rate: row.fake_rate,
                dmca_rate: row.dmca_rate,
                request_count: row.request_count,
                request_success_count: row.request_success_count,
                acquisition_count: row.acquisition_count,
                acquisition_success_count: row.acquisition_success_count,
                min_samples: row.min_samples,
                computed_at: row.computed_at,
            })
            .collect())
    }

    async fn indexer_health_event_list(
        &self,
        params: IndexerHealthEventListParams,
    ) -> Result<Vec<IndexerHealthEventResponse>, IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.health_event_list",
            actor_user_public_id = %params.actor_user_public_id,
            indexer_instance_public_id = %params.indexer_instance_public_id
        );
        let rows = self
            .run_data_operation(
                "indexer.health_event_list",
                "indexer_health_event_list",
                indexer_health_event_list(
                    self.config.pool(),
                    params.actor_user_public_id,
                    params.indexer_instance_public_id,
                    params.limit,
                )
                .instrument(span),
                map_indexer_instance_error,
            )
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| IndexerHealthEventResponse {
                occurred_at: row.occurred_at,
                event_type: row.event_type,
                latency_ms: row.latency_ms,
                http_status: row.http_status,
                error_class: row.error_class,
                detail: row.detail,
            })
            .collect())
    }

    async fn indexer_instance_test_prepare(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
    ) -> Result<IndexerInstanceTestPrepareResponse, IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.instance_test_prepare",
            actor_user_public_id = %actor_user_public_id,
            indexer_instance_public_id = %indexer_instance_public_id
        );
        let row = self
            .run_data_operation(
                "indexer.instance_test_prepare",
                "indexer_instance_test_prepare",
                indexer_instance_test_prepare(
                    self.config.pool(),
                    Some(actor_user_public_id),
                    indexer_instance_public_id,
                )
                .instrument(span),
                map_indexer_instance_error,
            )
            .await?;

        Ok(IndexerInstanceTestPrepareResponse {
            can_execute: row.can_execute,
            error_class: row.error_class,
            error_code: row.error_code,
            detail: row.detail,
            engine: row.engine,
            routing_policy_public_id: row.routing_policy_public_id,
            connect_timeout_ms: row.connect_timeout_ms,
            read_timeout_ms: row.read_timeout_ms,
            field_names: row.field_names,
            field_types: row.field_types,
            value_plain: row.value_plain,
            value_int: row.value_int,
            value_decimal: row.value_decimal,
            value_bool: row.value_bool,
            secret_public_ids: row.secret_public_ids,
        })
    }

    async fn indexer_instance_test_finalize(
        &self,
        params: IndexerInstanceTestFinalizeParams<'_>,
    ) -> Result<IndexerInstanceTestFinalizeResponse, IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.instance_test_finalize",
            actor_user_public_id = %params.actor_user_public_id,
            indexer_instance_public_id = %params.indexer_instance_public_id,
            ok = params.ok
        );
        let input = IndexerTestFinalizeInput {
            actor_user_public_id: Some(params.actor_user_public_id),
            indexer_instance_public_id: params.indexer_instance_public_id,
            ok: params.ok,
            error_class: params.error_class,
            error_code: params.error_code,
            detail: params.detail,
            result_count: params.result_count,
        };
        let row = self
            .run_data_operation(
                "indexer.instance_test_finalize",
                "indexer_instance_test_finalize",
                indexer_instance_test_finalize(self.config.pool(), &input).instrument(span),
                map_indexer_instance_error,
            )
            .await?;

        Ok(IndexerInstanceTestFinalizeResponse {
            ok: row.ok,
            error_class: row.error_class,
            error_code: row.error_code,
            detail: row.detail,
            result_count: row.result_count,
        })
    }

    async fn indexer_rss_subscription_get(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
    ) -> Result<IndexerRssSubscriptionResponse, IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.rss_subscription_get",
            actor_user_public_id = %actor_user_public_id,
            indexer_instance_public_id = %indexer_instance_public_id
        );
        let row = self
            .run_data_operation(
                "indexer.rss_subscription_get",
                "indexer_rss_subscription_get",
                rss_subscription_get(
                    self.config.pool(),
                    actor_user_public_id,
                    indexer_instance_public_id,
                )
                .instrument(span),
                map_indexer_instance_error,
            )
            .await?;

        Ok(IndexerRssSubscriptionResponse {
            indexer_instance_public_id: row.indexer_instance_public_id,
            instance_status: row.instance_status,
            rss_setting_status: row.rss_status,
            subscription_status: if !row.subscription_exists {
                "missing".to_string()
            } else if row.subscription_is_enabled {
                "enabled".to_string()
            } else {
                "disabled".to_string()
            },
            interval_seconds: row.interval_seconds,
            last_polled_at: row.last_polled_at,
            next_poll_at: row.next_poll_at,
            backoff_seconds: row.backoff_seconds,
            last_error_class: row.last_error_class,
        })
    }

    async fn indexer_rss_subscription_set(
        &self,
        params: IndexerRssSubscriptionParams,
    ) -> Result<IndexerRssSubscriptionResponse, IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.rss_subscription_set",
            actor_user_public_id = %params.actor_user_public_id,
            indexer_instance_public_id = %params.indexer_instance_public_id,
            subscription_enabled = params.is_enabled
        );
        self.run_data_operation(
            "indexer.rss_subscription_set",
            "indexer_rss_subscription_set",
            rss_subscription_set(
                self.config.pool(),
                params.actor_user_public_id,
                params.indexer_instance_public_id,
                params.is_enabled,
                params.interval_seconds,
            )
            .instrument(span),
            map_indexer_instance_error,
        )
        .await?;

        self.indexer_rss_subscription_get(
            params.actor_user_public_id,
            params.indexer_instance_public_id,
        )
        .await
    }

    async fn indexer_rss_seen_list(
        &self,
        params: IndexerRssSeenListParams,
    ) -> Result<Vec<IndexerRssSeenItemResponse>, IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.rss_seen_list",
            actor_user_public_id = %params.actor_user_public_id,
            indexer_instance_public_id = %params.indexer_instance_public_id
        );
        let rows = self
            .run_data_operation(
                "indexer.rss_seen_list",
                "indexer_rss_seen_list",
                rss_item_seen_list(
                    self.config.pool(),
                    params.actor_user_public_id,
                    params.indexer_instance_public_id,
                    params.limit,
                )
                .instrument(span),
                map_indexer_instance_error,
            )
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| IndexerRssSeenItemResponse {
                item_guid: row.item_guid,
                infohash_v1: row.infohash_v1,
                infohash_v2: row.infohash_v2,
                magnet_hash: row.magnet_hash,
                first_seen_at: row.first_seen_at,
            })
            .collect())
    }

    async fn indexer_rss_seen_mark(
        &self,
        params: IndexerRssSeenMarkParams<'_>,
    ) -> Result<IndexerRssSeenMarkResponse, IndexerInstanceServiceError> {
        let span = info_span!(
            "indexer.rss_seen_mark",
            actor_user_public_id = %params.actor_user_public_id,
            indexer_instance_public_id = %params.indexer_instance_public_id
        );
        let row = self
            .run_data_operation(
                "indexer.rss_seen_mark",
                "indexer_rss_seen_mark",
                rss_item_seen_mark(
                    self.config.pool(),
                    &RssSeenMarkInput {
                        actor_user_public_id: params.actor_user_public_id,
                        indexer_instance_public_id: params.indexer_instance_public_id,
                        item_guid: params.item_guid,
                        infohash_v1: params.infohash_v1,
                        infohash_v2: params.infohash_v2,
                        magnet_hash: params.magnet_hash,
                    },
                )
                .instrument(span),
                map_indexer_instance_error,
            )
            .await?;

        Ok(IndexerRssSeenMarkResponse {
            item: IndexerRssSeenItemResponse {
                item_guid: row.item_guid,
                infohash_v1: row.infohash_v1,
                infohash_v2: row.infohash_v2,
                magnet_hash: row.magnet_hash,
                first_seen_at: row.first_seen_at,
            },
            inserted: row.inserted,
        })
    }

    async fn secret_create(
        &self,
        actor_user_public_id: Uuid,
        secret_type: &str,
        secret_value: &str,
    ) -> Result<Uuid, SecretServiceError> {
        let span = info_span!(
            "secret.create",
            actor_user_public_id = %actor_user_public_id,
            secret_type = secret_type
        );
        self.run_data_operation(
            "indexer.secret_create",
            "secret_create",
            secret_create(
                self.config.pool(),
                actor_user_public_id,
                secret_type,
                secret_value,
            )
            .instrument(span),
            map_secret_error,
        )
        .await
    }

    async fn secret_metadata_list(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<Vec<SecretMetadataResponse>, SecretServiceError> {
        let span = info_span!(
            "secret.metadata_list",
            actor_user_public_id = %actor_user_public_id
        );
        let rows = self
            .run_data_operation(
                "indexer.secret_metadata_list",
                "secret_metadata_list",
                secret_metadata_list(self.config.pool(), actor_user_public_id).instrument(span),
                map_secret_error,
            )
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| SecretMetadataResponse {
                secret_public_id: row.secret_public_id,
                secret_type: row.secret_type,
                is_revoked: row.is_revoked,
                created_at: row.created_at,
                rotated_at: row.rotated_at,
                binding_count: row.binding_count,
            })
            .collect())
    }

    async fn secret_rotate(
        &self,
        actor_user_public_id: Uuid,
        secret_public_id: Uuid,
        secret_value: &str,
    ) -> Result<Uuid, SecretServiceError> {
        let span = info_span!(
            "secret.rotate",
            actor_user_public_id = %actor_user_public_id,
            secret_public_id = %secret_public_id
        );
        self.run_data_operation(
            "indexer.secret_rotate",
            "secret_rotate",
            secret_rotate(
                self.config.pool(),
                actor_user_public_id,
                secret_public_id,
                secret_value,
            )
            .instrument(span),
            map_secret_error,
        )
        .await
    }

    async fn secret_revoke(
        &self,
        actor_user_public_id: Uuid,
        secret_public_id: Uuid,
    ) -> Result<(), SecretServiceError> {
        let span = info_span!(
            "secret.revoke",
            actor_user_public_id = %actor_user_public_id,
            secret_public_id = %secret_public_id
        );
        self.run_data_operation(
            "indexer.secret_revoke",
            "secret_revoke",
            secret_revoke(self.config.pool(), actor_user_public_id, secret_public_id)
                .instrument(span),
            map_secret_error,
        )
        .await
    }
}
fn map_indexer_definition_row(row: IndexerDefinitionRow) -> IndexerDefinitionResponse {
    IndexerDefinitionResponse {
        upstream_source: row.upstream_source,
        upstream_slug: row.upstream_slug,
        display_name: row.display_name,
        protocol: row.protocol,
        engine: row.engine,
        schema_version: row.schema_version,
        definition_hash: row.definition_hash,
        is_deprecated: row.is_deprecated,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn map_imported_indexer_definition_row(
    row: ImportedIndexerDefinitionRow,
) -> CardigannDefinitionImportResponse {
    CardigannDefinitionImportResponse {
        definition: IndexerDefinitionResponse {
            upstream_source: row.upstream_source,
            upstream_slug: row.upstream_slug,
            display_name: row.display_name,
            protocol: row.protocol,
            engine: row.engine,
            schema_version: row.schema_version,
            definition_hash: row.definition_hash,
            is_deprecated: row.is_deprecated,
            created_at: row.created_at,
            updated_at: row.updated_at,
        },
        field_count: row.field_count,
        option_count: row.option_count,
    }
}

fn parse_cardigann_definition_import(
    yaml_payload: &str,
) -> Result<PreparedCardigannDefinitionImport, IndexerDefinitionServiceError> {
    let trimmed_payload = yaml_payload.trim();
    if trimmed_payload.is_empty() {
        return Err(invalid_indexer_definition_error(
            "cardigann_yaml_payload_missing",
        ));
    }

    let document: CardigannDefinitionDocument = serde_yaml::from_str(trimmed_payload)
        .map_err(|_| invalid_indexer_definition_error("cardigann_yaml_invalid"))?;
    let upstream_slug = normalize_cardigann_slug(&document.id)?;
    let display_name = normalize_cardigann_display_name(&document.name)?;

    let mut canonical_settings = Vec::with_capacity(document.settings.len());
    let mut prepared_fields = Vec::with_capacity(document.settings.len());
    for (index, setting) in document.settings.iter().enumerate() {
        let prepared_field = prepare_cardigann_field(setting, index)?;
        let canonical_options = prepared_field
            .option_values
            .iter()
            .zip(prepared_field.option_labels.iter())
            .map(|(value, label)| CanonicalCardigannSettingOption {
                value: value.clone(),
                label: label.clone(),
            })
            .collect::<Vec<_>>();
        let default_value = cardigann_default_string(setting.default.as_ref());
        canonical_settings.push(CanonicalCardigannSetting {
            name: prepared_field.field_name.clone(),
            label: prepared_field.label.clone(),
            setting_type: prepared_field.field_type.clone(),
            required: prepared_field.is_required,
            advanced: prepared_field.is_advanced,
            default_value,
            options: canonical_options,
        });
        prepared_fields.push(prepared_field);
    }

    let canonical_definition = CanonicalCardigannDefinition {
        id: upstream_slug.clone(),
        name: display_name.clone(),
        description: document.description.clone(),
        caps: document.caps.clone(),
        login: document.login.clone(),
        search: document.search.clone(),
        test: document.test,
        settings: canonical_settings,
    };
    let canonical_definition_text = to_json_string(&canonical_definition)
        .map_err(|_| invalid_indexer_definition_error("cardigann_canonicalization_failed"))?;

    Ok(PreparedCardigannDefinitionImport {
        upstream_slug,
        display_name,
        canonical_definition_text,
        fields: prepared_fields,
    })
}

fn prepare_cardigann_field(
    setting: &CardigannSettingDocument,
    index: usize,
) -> Result<PreparedCardigannFieldImport, IndexerDefinitionServiceError> {
    let field_name = normalize_cardigann_field_name(&setting.name)?;
    let label = setting
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map_or_else(|| field_name.clone(), ToOwned::to_owned);
    let field_type = map_cardigann_setting_type(&setting.setting_type)?;
    let display_order = i32::try_from(index + 1)
        .map_err(|_| invalid_indexer_definition_error("cardigann_setting_order_invalid"))?;
    let option_values = setting
        .options
        .iter()
        .map(cardigann_option_value)
        .collect::<Result<Vec<_>, _>>()?;
    let option_labels = setting
        .options
        .iter()
        .map(cardigann_option_label)
        .collect::<Result<Vec<_>, _>>()?;

    let mut field = PreparedCardigannFieldImport {
        field_name,
        label,
        field_type,
        is_required: setting.required.unwrap_or(false),
        is_advanced: setting.advanced.unwrap_or(false),
        display_order,
        default_value_plain: None,
        default_value_int: None,
        default_value_decimal: None,
        default_value_bool: None,
        option_values,
        option_labels,
    };
    populate_cardigann_default(&mut field, setting.default.as_ref())?;
    Ok(field)
}

fn populate_cardigann_default(
    field: &mut PreparedCardigannFieldImport,
    value: Option<&YamlValue>,
) -> Result<(), IndexerDefinitionServiceError> {
    let Some(value) = value else {
        return Ok(());
    };

    match field.field_type.as_str() {
        "bool" => {
            let Some(value) = value.as_bool() else {
                return Err(invalid_indexer_definition_error(
                    "cardigann_setting_default_invalid",
                ));
            };
            field.default_value_bool = Some(value);
        }
        "number_int" => {
            let Some(value) = value.as_i64() else {
                return Err(invalid_indexer_definition_error(
                    "cardigann_setting_default_invalid",
                ));
            };
            let value = i32::try_from(value).map_err(|_| {
                invalid_indexer_definition_error("cardigann_setting_default_invalid")
            })?;
            field.default_value_int = Some(value);
        }
        "number_decimal" => {
            let Some(number) = value.as_f64() else {
                return Err(invalid_indexer_definition_error(
                    "cardigann_setting_default_invalid",
                ));
            };
            field.default_value_decimal = Some(number.to_string());
        }
        _ => {
            let Some(value) = cardigann_default_string(Some(value)) else {
                return Err(invalid_indexer_definition_error(
                    "cardigann_setting_default_invalid",
                ));
            };
            field.default_value_plain = Some(value);
        }
    }
    Ok(())
}

fn cardigann_default_string(value: Option<&YamlValue>) -> Option<String> {
    let value = value?;
    match value {
        YamlValue::Bool(value) => Some(value.to_string()),
        YamlValue::Number(value) => Some(value.to_string()),
        YamlValue::String(value) => Some(value.trim().to_string()),
        _ => None,
    }
}

fn normalize_cardigann_slug(slug: &str) -> Result<String, IndexerDefinitionServiceError> {
    let normalized = slug.trim().to_lowercase();
    if normalized.is_empty() {
        return Err(invalid_indexer_definition_error(
            "cardigann_definition_slug_missing",
        ));
    }
    Ok(normalized)
}

fn normalize_cardigann_display_name(
    display_name: &str,
) -> Result<String, IndexerDefinitionServiceError> {
    let normalized = display_name.trim().to_string();
    if normalized.is_empty() {
        return Err(invalid_indexer_definition_error(
            "cardigann_definition_name_missing",
        ));
    }
    Ok(normalized)
}

fn normalize_cardigann_field_name(
    field_name: &str,
) -> Result<String, IndexerDefinitionServiceError> {
    let normalized = field_name.trim().to_lowercase();
    if normalized.is_empty() {
        return Err(invalid_indexer_definition_error(
            "cardigann_setting_name_missing",
        ));
    }
    Ok(normalized)
}

fn map_cardigann_setting_type(setting_type: &str) -> Result<String, IndexerDefinitionServiceError> {
    match setting_type.trim().to_lowercase().as_str() {
        "text" | "info" | "hidden" => Ok("string".to_string()),
        "password" => Ok("password".to_string()),
        "apikey" | "api_key" => Ok("api_key".to_string()),
        "cookie" => Ok("cookie".to_string()),
        "token" | "captcha" => Ok("token".to_string()),
        "number" | "integer" => Ok("number_int".to_string()),
        "float" | "decimal" => Ok("number_decimal".to_string()),
        "checkbox" | "bool" | "boolean" => Ok("bool".to_string()),
        "select" => Ok("select_single".to_string()),
        _ => Err(invalid_indexer_definition_error(
            "cardigann_setting_type_unsupported",
        )),
    }
}

fn cardigann_option_value(
    option: &CardigannSettingOptionDocument,
) -> Result<String, IndexerDefinitionServiceError> {
    match option {
        CardigannSettingOptionDocument::Simple(value)
        | CardigannSettingOptionDocument::Labeled { value, .. } => {
            let normalized = value.trim().to_string();
            if normalized.is_empty() {
                return Err(invalid_indexer_definition_error(
                    "cardigann_setting_option_invalid",
                ));
            }
            Ok(normalized)
        }
    }
}

fn cardigann_option_label(
    option: &CardigannSettingOptionDocument,
) -> Result<String, IndexerDefinitionServiceError> {
    match option {
        CardigannSettingOptionDocument::Simple(value) => {
            let normalized = value.trim().to_string();
            if normalized.is_empty() {
                return Err(invalid_indexer_definition_error(
                    "cardigann_setting_option_invalid",
                ));
            }
            Ok(normalized)
        }
        CardigannSettingOptionDocument::Labeled { value, label, name } => {
            let normalized = label
                .as_deref()
                .or(name.as_deref())
                .unwrap_or(value)
                .trim()
                .to_string();
            if normalized.is_empty() {
                return Err(invalid_indexer_definition_error(
                    "cardigann_setting_option_invalid",
                ));
            }
            Ok(normalized)
        }
    }
}

fn invalid_indexer_definition_error(code: &'static str) -> IndexerDefinitionServiceError {
    IndexerDefinitionServiceError::new(IndexerDefinitionServiceErrorKind::Invalid).with_code(code)
}

fn map_health_notification_hook_row(
    row: IndexerHealthNotificationHookRow,
) -> IndexerHealthNotificationHookResponse {
    IndexerHealthNotificationHookResponse {
        indexer_health_notification_hook_public_id: row.indexer_health_notification_hook_public_id,
        channel: row.channel,
        display_name: row.display_name,
        status_threshold: row.status_threshold,
        webhook_url: row.webhook_url,
        email: row.email,
        is_enabled: row.is_enabled,
        updated_at: row.updated_at,
    }
}

fn map_indexer_definition_error(
    _operation: &'static str,
    error: &DataError,
) -> IndexerDefinitionServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = indexer_definition_error_kind(code.as_deref());

    let mut service_error = IndexerDefinitionServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn indexer_definition_error_kind(detail: Option<&str>) -> IndexerDefinitionServiceErrorKind {
    match detail {
        Some(
            "definition_upstream_slug_missing"
            | "definition_display_name_missing"
            | "definition_canonical_text_missing"
            | "definition_field_name_missing"
            | "definition_field_label_missing"
            | "definition_option_length_mismatch",
        ) => IndexerDefinitionServiceErrorKind::Invalid,
        Some("actor_missing" | "actor_not_found" | "actor_unauthorized") => {
            IndexerDefinitionServiceErrorKind::Unauthorized
        }
        _ => IndexerDefinitionServiceErrorKind::Storage,
    }
}

fn map_tag_error(_operation: &'static str, error: &DataError) -> TagServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = tag_error_kind(code.as_deref());

    let mut service_error = TagServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn tag_error_kind(detail: Option<&str>) -> TagServiceErrorKind {
    match detail {
        Some("tag_not_found" | "unknown_key") => TagServiceErrorKind::NotFound,
        Some("tag_key_already_exists" | "tag_deleted") => TagServiceErrorKind::Conflict,
        Some("actor_missing" | "actor_not_found") => TagServiceErrorKind::Unauthorized,
        Some(
            "tag_reference_missing"
            | "tag_key_missing"
            | "tag_key_empty"
            | "tag_key_not_lowercase"
            | "tag_key_too_long"
            | "display_name_missing"
            | "display_name_empty"
            | "display_name_too_long"
            | "invalid_tag_reference",
        ) => TagServiceErrorKind::Invalid,
        _ => TagServiceErrorKind::Storage,
    }
}

fn map_health_notification_error(
    _operation: &'static str,
    error: &DataError,
) -> HealthNotificationServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = health_notification_error_kind(code.as_deref());

    let mut service_error = HealthNotificationServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn health_notification_error_kind(detail: Option<&str>) -> HealthNotificationServiceErrorKind {
    match detail {
        Some("hook_not_found") => HealthNotificationServiceErrorKind::NotFound,
        Some("actor_missing" | "actor_not_found" | "actor_unauthorized") => {
            HealthNotificationServiceErrorKind::Unauthorized
        }
        Some(
            "channel_invalid"
            | "display_name_missing"
            | "status_threshold_missing"
            | "webhook_url_missing"
            | "webhook_url_invalid"
            | "email_missing"
            | "email_invalid"
            | "hook_missing"
            | "channel_payload_mismatch",
        ) => HealthNotificationServiceErrorKind::Invalid,
        _ => HealthNotificationServiceErrorKind::Storage,
    }
}

fn map_routing_policy_error(
    _operation: &'static str,
    error: &DataError,
) -> RoutingPolicyServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = routing_policy_error_kind(code.as_deref());

    let mut service_error = RoutingPolicyServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn routing_policy_error_kind(detail: Option<&str>) -> RoutingPolicyServiceErrorKind {
    match detail {
        Some("routing_policy_not_found" | "secret_not_found") => {
            RoutingPolicyServiceErrorKind::NotFound
        }
        Some("routing_policy_deleted" | "display_name_already_exists") => {
            RoutingPolicyServiceErrorKind::Conflict
        }
        Some("actor_missing" | "actor_not_found" | "actor_unauthorized") => {
            RoutingPolicyServiceErrorKind::Unauthorized
        }
        Some(
            "display_name_missing"
            | "display_name_empty"
            | "display_name_too_long"
            | "mode_missing"
            | "unsupported_routing_mode"
            | "routing_policy_missing"
            | "param_key_missing"
            | "param_not_allowed"
            | "param_requires_secret"
            | "param_value_invalid"
            | "param_value_out_of_range"
            | "param_value_too_long"
            | "secret_missing",
        ) => RoutingPolicyServiceErrorKind::Invalid,
        _ => RoutingPolicyServiceErrorKind::Storage,
    }
}

fn map_rate_limit_policy_error(
    _operation: &'static str,
    error: &DataError,
) -> RateLimitPolicyServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = rate_limit_policy_error_kind(code.as_deref());

    let mut service_error = RateLimitPolicyServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn rate_limit_policy_error_kind(detail: Option<&str>) -> RateLimitPolicyServiceErrorKind {
    match detail {
        Some("policy_not_found" | "indexer_not_found" | "routing_policy_not_found") => {
            RateLimitPolicyServiceErrorKind::NotFound
        }
        Some(
            "display_name_already_exists"
            | "policy_is_system"
            | "policy_in_use"
            | "policy_deleted"
            | "indexer_deleted"
            | "routing_policy_deleted",
        ) => RateLimitPolicyServiceErrorKind::Conflict,
        Some("actor_missing" | "actor_not_found" | "actor_unauthorized") => {
            RateLimitPolicyServiceErrorKind::Unauthorized
        }
        Some(
            "display_name_missing"
            | "display_name_empty"
            | "display_name_too_long"
            | "limit_missing"
            | "rpm_out_of_range"
            | "burst_out_of_range"
            | "concurrent_out_of_range"
            | "policy_missing"
            | "indexer_missing"
            | "routing_policy_missing"
            | "scope_missing"
            | "scope_id_missing"
            | "capacity_invalid"
            | "tokens_invalid",
        ) => RateLimitPolicyServiceErrorKind::Invalid,
        _ => RateLimitPolicyServiceErrorKind::Storage,
    }
}

fn map_search_profile_error(
    _operation: &'static str,
    error: &DataError,
) -> SearchProfileServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = search_profile_error_kind(code.as_deref());

    let mut service_error = SearchProfileServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn search_profile_error_kind(detail: Option<&str>) -> SearchProfileServiceErrorKind {
    match detail {
        Some(
            "search_profile_not_found"
            | "media_domain_not_found"
            | "policy_set_not_found"
            | "indexer_not_found"
            | "tag_not_found"
            | "user_not_found",
        ) => SearchProfileServiceErrorKind::NotFound,
        Some(
            "search_profile_deleted"
            | "policy_set_deleted"
            | "indexer_block_conflict"
            | "indexer_allow_conflict"
            | "tag_block_conflict"
            | "tag_allow_conflict",
        ) => SearchProfileServiceErrorKind::Conflict,
        Some("actor_missing" | "actor_not_found" | "actor_unauthorized") => {
            SearchProfileServiceErrorKind::Unauthorized
        }
        Some(
            "display_name_missing"
            | "display_name_empty"
            | "display_name_too_long"
            | "search_profile_missing"
            | "policy_set_missing"
            | "policy_set_invalid_scope"
            | "media_domain_key_invalid"
            | "indexer_id_invalid"
            | "tag_key_invalid"
            | "invalid_tag_reference"
            | "default_not_in_allowlist"
            | "unknown_key",
        ) => SearchProfileServiceErrorKind::Invalid,
        _ => SearchProfileServiceErrorKind::Storage,
    }
}

fn map_search_request_error(
    _operation: &'static str,
    error: &DataError,
) -> SearchRequestServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = search_request_error_kind(code.as_deref());

    let mut service_error = SearchRequestServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn search_request_error_kind(detail: Option<&str>) -> SearchRequestServiceErrorKind {
    match detail {
        Some(
            "search_request_not_found"
            | "search_profile_not_found"
            | "media_domain_not_found"
            | "search_page_not_found",
        ) => SearchRequestServiceErrorKind::NotFound,
        Some("actor_missing" | "actor_not_found" | "actor_unauthorized") => {
            SearchRequestServiceErrorKind::Unauthorized
        }
        Some(
            "query_text_missing"
            | "query_text_too_long"
            | "identifier_input_invalid"
            | "invalid_identifier_combo"
            | "invalid_query"
            | "invalid_season_episode_combo"
            | "invalid_torznab_mode"
            | "invalid_identifier_mismatch"
            | "query_type_missing"
            | "media_domain_key_invalid"
            | "invalid_request_policy_set"
            | "invalid_category_filter"
            | "search_request_missing"
            | "page_number_missing"
            | "page_number_invalid",
        ) => SearchRequestServiceErrorKind::Invalid,
        _ => SearchRequestServiceErrorKind::Storage,
    }
}

const fn map_search_page_summary(row: &SearchPageSummaryRow) -> SearchPageSummaryResponse {
    SearchPageSummaryResponse {
        page_number: row.page_number,
        sealed_at: row.sealed_at,
        item_count: row.item_count,
    }
}

fn map_search_page_item(row: &SearchPageFetchRow) -> Option<SearchPageItemResponse> {
    let position = row.item_position?;
    let canonical_torrent_public_id = row.canonical_torrent_public_id?;
    let title_display = row.title_display.clone()?;

    Some(SearchPageItemResponse {
        position,
        canonical_torrent_public_id,
        title_display,
        size_bytes: row.size_bytes,
        infohash_v1: row.infohash_v1.clone(),
        infohash_v2: row.infohash_v2.clone(),
        magnet_hash: row.magnet_hash.clone(),
        canonical_torrent_source_public_id: row.canonical_torrent_source_public_id,
        indexer_instance_public_id: row.indexer_instance_public_id,
        indexer_display_name: row.indexer_display_name.clone(),
        seeders: row.seeders,
        leechers: row.leechers,
        published_at: row.published_at,
        download_url: row.download_url.clone(),
        magnet_uri: row.magnet_uri.clone(),
        details_url: row.details_url.clone(),
        tracker_name: row.tracker_name.clone(),
        tracker_category: row.tracker_category,
        tracker_subcategory: row.tracker_subcategory,
    })
}

fn map_search_request_explainability(
    row: &SearchRequestExplainabilityRow,
) -> SearchRequestExplainabilityResponse {
    SearchRequestExplainabilityResponse {
        zero_runnable_indexers: row.zero_runnable_indexers,
        skipped_canceled_indexers: row.skipped_canceled_indexers,
        skipped_failed_indexers: row.skipped_failed_indexers,
        blocked_results: row.blocked_results,
        blocked_rule_public_ids: row.blocked_rule_public_ids.clone(),
        rate_limited_indexers: row.rate_limited_indexers,
        retrying_indexers: row.retrying_indexers,
    }
}

fn map_import_job_status(status: ImportJobStatusRow) -> ImportJobStatusResponse {
    ImportJobStatusResponse {
        status: status.status,
        result_total: status.result_total,
        result_imported_ready: status.result_imported_ready,
        result_imported_needs_secret: status.result_imported_needs_secret,
        result_imported_test_failed: status.result_imported_test_failed,
        result_unmapped_definition: status.result_unmapped_definition,
        result_skipped_duplicate: status.result_skipped_duplicate,
    }
}

fn map_import_job_result(result: ImportJobResultRow) -> ImportJobResultResponse {
    ImportJobResultResponse {
        prowlarr_identifier: result.prowlarr_identifier,
        upstream_slug: result.upstream_slug,
        indexer_instance_public_id: result.indexer_instance_public_id,
        status: result.status,
        detail: result.detail,
        resolved_is_enabled: result.resolved_is_enabled,
        resolved_priority: result.resolved_priority,
        missing_secret_fields: result.missing_secret_fields,
        media_domain_keys: result.media_domain_keys,
        tag_keys: result.tag_keys,
        created_at: result.created_at,
    }
}

fn map_source_metadata_conflict(
    row: SourceMetadataConflictRow,
) -> IndexerSourceMetadataConflictResponse {
    IndexerSourceMetadataConflictResponse {
        conflict_id: row.conflict_id,
        conflict_type: row.conflict_type,
        existing_value: row.existing_value,
        incoming_value: row.incoming_value,
        observed_at: row.observed_at,
        resolved_at: row.resolved_at,
        resolution: row.resolution,
        resolution_note: row.resolution_note,
    }
}

fn map_import_job_error(_operation: &'static str, error: &DataError) -> ImportJobServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = import_job_error_kind(code.as_deref());

    let mut service_error = ImportJobServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn import_job_error_kind(detail: Option<&str>) -> ImportJobServiceErrorKind {
    match detail {
        Some(
            "import_job_not_found"
            | "search_profile_not_found"
            | "torznab_instance_not_found"
            | "secret_not_found",
        ) => ImportJobServiceErrorKind::NotFound,
        Some("import_job_not_startable" | "import_source_mismatch") => {
            ImportJobServiceErrorKind::Conflict
        }
        Some("actor_missing" | "actor_not_found" | "actor_unauthorized") => {
            ImportJobServiceErrorKind::Unauthorized
        }
        Some(
            "import_job_missing"
            | "source_missing"
            | "prowlarr_url_missing"
            | "prowlarr_url_too_long"
            | "secret_missing"
            | "backup_blob_missing"
            | "backup_blob_too_long"
            | "config_too_long",
        ) => ImportJobServiceErrorKind::Invalid,
        _ => ImportJobServiceErrorKind::Storage,
    }
}

fn map_source_metadata_conflict_error(error: &DataError) -> SourceMetadataConflictServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = match code.as_deref() {
        Some("conflict_not_found") => SourceMetadataConflictServiceErrorKind::NotFound,
        Some("conflict_already_resolved" | "conflict_not_resolved" | "source_guid_conflict") => {
            SourceMetadataConflictServiceErrorKind::Conflict
        }
        Some("actor_missing" | "actor_not_found" | "actor_unauthorized") => {
            SourceMetadataConflictServiceErrorKind::Unauthorized
        }
        Some(
            "conflict_missing"
            | "resolution_missing"
            | "resolution_note_too_long"
            | "incoming_value_invalid"
            | "limit_invalid",
        ) => SourceMetadataConflictServiceErrorKind::Invalid,
        _ => SourceMetadataConflictServiceErrorKind::Storage,
    };

    let mut service_error = SourceMetadataConflictServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn build_backup_snapshot(
    tags: Vec<BackupTagRow>,
    rate_limits: Vec<BackupRateLimitPolicyRow>,
    routing_rows: &[BackupRoutingPolicyRow],
    instance_rows: &[BackupIndexerInstanceRow],
) -> IndexerBackupSnapshot {
    let mut secret_refs = BTreeMap::new();
    let routing_policies = build_backup_routing_policies(routing_rows, &mut secret_refs);
    let indexer_instances = build_backup_indexer_instances(instance_rows, &mut secret_refs);

    IndexerBackupSnapshot {
        version: "revaer.indexers.backup.v1".to_string(),
        exported_at: Utc::now(),
        tags: tags
            .into_iter()
            .map(|tag| IndexerBackupTagItem {
                tag_key: tag.tag_key,
                display_name: tag.display_name,
            })
            .collect(),
        rate_limit_policies: rate_limits
            .into_iter()
            .map(|policy| IndexerBackupRateLimitPolicyItem {
                display_name: policy.display_name,
                requests_per_minute: policy.requests_per_minute,
                burst: policy.burst,
                concurrent_requests: policy.concurrent_requests,
                is_system: policy.is_system,
            })
            .collect(),
        routing_policies,
        indexer_instances,
        secrets: secret_refs.into_values().collect(),
    }
}

fn build_routing_policy_inventory(
    rows: &[BackupRoutingPolicyRow],
) -> Vec<RoutingPolicyListItemResponse> {
    let mut policies = BTreeMap::<Uuid, RoutingPolicyListItemResponse>::new();

    for row in rows {
        let entry = policies
            .entry(row.routing_policy_public_id)
            .or_insert_with(|| RoutingPolicyListItemResponse {
                routing_policy_public_id: row.routing_policy_public_id,
                display_name: row.display_name.clone(),
                mode: row.mode.clone(),
                rate_limit_policy_public_id: row.rate_limit_policy_public_id,
                rate_limit_display_name: row.rate_limit_display_name.clone(),
                parameter_count: 0,
                secret_binding_count: 0,
            });

        if row.param_key.is_some() {
            entry.parameter_count = entry.parameter_count.saturating_add(1);
        }
        if row.secret_public_id.is_some() {
            entry.secret_binding_count = entry.secret_binding_count.saturating_add(1);
        }
        if entry.rate_limit_policy_public_id.is_none() {
            entry.rate_limit_policy_public_id = row.rate_limit_policy_public_id;
        }
        if entry.rate_limit_display_name.is_none() {
            entry
                .rate_limit_display_name
                .clone_from(&row.rate_limit_display_name);
        }
    }

    let mut items = policies.into_values().collect::<Vec<_>>();
    items.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    items
}

fn build_indexer_instance_inventory(
    rows: &[BackupIndexerInstanceRow],
) -> Vec<IndexerInstanceListItemResponse> {
    let mut instances = BTreeMap::<Uuid, IndexerInstanceListItemResponse>::new();
    let mut media_domain_keys = BTreeMap::<Uuid, BTreeSet<String>>::new();
    let mut tag_keys = BTreeMap::<Uuid, BTreeSet<String>>::new();
    let mut fields =
        BTreeMap::<Uuid, BTreeMap<String, IndexerInstanceFieldInventoryResponse>>::new();

    for row in rows {
        update_indexer_instance_inventory_entry(&mut instances, row);
        insert_indexer_instance_inventory_media_domain(&mut media_domain_keys, row);
        insert_indexer_instance_inventory_tag(&mut tag_keys, row);
        insert_indexer_instance_inventory_field(&mut fields, row);
    }

    for (indexer_instance_public_id, media_domain_set) in media_domain_keys {
        if let Some(instance) = instances.get_mut(&indexer_instance_public_id) {
            instance.media_domain_keys = media_domain_set.into_iter().collect();
        }
    }
    for (indexer_instance_public_id, tag_set) in tag_keys {
        if let Some(instance) = instances.get_mut(&indexer_instance_public_id) {
            instance.tag_keys = tag_set.into_iter().collect();
        }
    }
    for (indexer_instance_public_id, field_map) in fields {
        if let Some(instance) = instances.get_mut(&indexer_instance_public_id) {
            instance.fields = field_map.into_values().collect();
        }
    }

    let mut items = instances.into_values().collect::<Vec<_>>();
    items.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    items
}

fn build_search_profile_inventory_item(row: SearchProfileListRow) -> SearchProfileListItemResponse {
    SearchProfileListItemResponse {
        search_profile_public_id: row.search_profile_public_id,
        display_name: row.display_name,
        is_default: row.is_default,
        page_size: row.page_size,
        default_media_domain_key: row.default_media_domain_key,
        media_domain_keys: row.media_domain_keys,
        policy_set_public_ids: row.policy_set_public_ids,
        policy_set_display_names: row.policy_set_display_names,
        allow_indexer_public_ids: row.allow_indexer_public_ids,
        block_indexer_public_ids: row.block_indexer_public_ids,
        allow_tag_keys: row.allow_tag_keys,
        block_tag_keys: row.block_tag_keys,
        prefer_tag_keys: row.prefer_tag_keys,
    }
}

fn build_policy_set_inventory(rows: &[PolicySetRuleListRow]) -> Vec<PolicySetListItemResponse> {
    let mut policy_sets = BTreeMap::<Uuid, PolicySetListItemResponse>::new();

    for row in rows {
        let entry = policy_sets
            .entry(row.policy_set_public_id)
            .or_insert_with(|| PolicySetListItemResponse {
                policy_set_public_id: row.policy_set_public_id,
                display_name: row.policy_set_display_name.clone(),
                scope: row.scope.clone(),
                is_enabled: row.is_enabled,
                user_public_id: row.user_public_id,
                rules: Vec::new(),
            });

        if let (
            Some(policy_rule_public_id),
            Some(rule_type),
            Some(match_field),
            Some(match_operator),
            Some(sort_order),
            Some(action),
            Some(severity),
        ) = (
            row.policy_rule_public_id,
            row.rule_type.as_ref(),
            row.match_field.as_ref(),
            row.match_operator.as_ref(),
            row.sort_order,
            row.action.as_ref(),
            row.severity.as_ref(),
        ) {
            entry.rules.push(PolicyRuleListItemResponse {
                policy_rule_public_id,
                rule_type: rule_type.clone(),
                match_field: match_field.clone(),
                match_operator: match_operator.clone(),
                sort_order,
                match_value_text: row.match_value_text.clone(),
                match_value_int: row.match_value_int,
                match_value_uuid: row.match_value_uuid,
                action: action.clone(),
                severity: severity.clone(),
                is_case_insensitive: row.is_case_insensitive.unwrap_or(false),
                rationale: row.rationale.clone(),
                expires_at: row.expires_at,
                is_disabled: row.is_rule_disabled.unwrap_or(false),
            });
        }
    }

    let mut items = policy_sets.into_values().collect::<Vec<_>>();
    items.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    for item in &mut items {
        item.rules.sort_by(|left, right| {
            left.sort_order
                .cmp(&right.sort_order)
                .then_with(|| left.policy_rule_public_id.cmp(&right.policy_rule_public_id))
        });
    }
    items
}

fn build_torznab_instance_inventory_item(
    row: TorznabInstanceListRow,
) -> TorznabInstanceListItemResponse {
    TorznabInstanceListItemResponse {
        torznab_instance_public_id: row.torznab_instance_public_id,
        display_name: row.display_name,
        is_enabled: row.is_enabled,
        search_profile_public_id: row.search_profile_public_id,
        search_profile_display_name: row.search_profile_display_name,
    }
}

fn update_indexer_instance_inventory_entry(
    instances: &mut BTreeMap<Uuid, IndexerInstanceListItemResponse>,
    row: &BackupIndexerInstanceRow,
) {
    let entry = instances
        .entry(row.indexer_instance_public_id)
        .or_insert_with(|| IndexerInstanceListItemResponse {
            indexer_instance_public_id: row.indexer_instance_public_id,
            upstream_slug: row.upstream_slug.clone(),
            display_name: row.display_name.clone(),
            instance_status: row.instance_status.clone(),
            rss_status: row.rss_status.clone(),
            automatic_search_status: row.automatic_search_status.clone(),
            interactive_search_status: row.interactive_search_status.clone(),
            priority: row.priority,
            trust_tier_key: row.trust_tier_key.clone(),
            routing_policy_public_id: row.routing_policy_public_id,
            routing_policy_display_name: row.routing_policy_display_name.clone(),
            connect_timeout_ms: row.connect_timeout_ms,
            read_timeout_ms: row.read_timeout_ms,
            max_parallel_requests: row.max_parallel_requests,
            rate_limit_policy_public_id: row.rate_limit_policy_public_id,
            rate_limit_display_name: row.rate_limit_display_name.clone(),
            rss_subscription_enabled: row.rss_subscription_enabled,
            rss_interval_seconds: row.rss_interval_seconds,
            media_domain_keys: Vec::new(),
            tag_keys: Vec::new(),
            fields: Vec::new(),
        });

    if entry.routing_policy_public_id.is_none() {
        entry.routing_policy_public_id = row.routing_policy_public_id;
    }
    if entry.routing_policy_display_name.is_none() {
        entry
            .routing_policy_display_name
            .clone_from(&row.routing_policy_display_name);
    }
    if entry.rate_limit_policy_public_id.is_none() {
        entry.rate_limit_policy_public_id = row.rate_limit_policy_public_id;
    }
    if entry.rate_limit_display_name.is_none() {
        entry
            .rate_limit_display_name
            .clone_from(&row.rate_limit_display_name);
    }
    if entry.rss_subscription_enabled.is_none() {
        entry.rss_subscription_enabled = row.rss_subscription_enabled;
    }
    if entry.rss_interval_seconds.is_none() {
        entry.rss_interval_seconds = row.rss_interval_seconds;
    }
}

fn insert_indexer_instance_inventory_media_domain(
    media_domain_keys: &mut BTreeMap<Uuid, BTreeSet<String>>,
    row: &BackupIndexerInstanceRow,
) {
    if let Some(media_domain_key) = row.media_domain_key.clone() {
        media_domain_keys
            .entry(row.indexer_instance_public_id)
            .or_default()
            .insert(media_domain_key);
    }
}

fn insert_indexer_instance_inventory_tag(
    tag_keys: &mut BTreeMap<Uuid, BTreeSet<String>>,
    row: &BackupIndexerInstanceRow,
) {
    if let Some(tag_key) = row.tag_key.clone() {
        tag_keys
            .entry(row.indexer_instance_public_id)
            .or_default()
            .insert(tag_key);
    }
}

fn insert_indexer_instance_inventory_field(
    fields: &mut BTreeMap<Uuid, BTreeMap<String, IndexerInstanceFieldInventoryResponse>>,
    row: &BackupIndexerInstanceRow,
) {
    let Some(field_name) = row.field_name.clone() else {
        return;
    };

    fields
        .entry(row.indexer_instance_public_id)
        .or_default()
        .insert(
            field_name.clone(),
            IndexerInstanceFieldInventoryResponse {
                field_name,
                field_type: row.field_type.clone().unwrap_or_default(),
                value_plain: row.value_plain.clone(),
                value_int: row.value_int,
                value_decimal: row.value_decimal.clone(),
                value_bool: row.value_bool,
                secret_public_id: row.secret_public_id,
            },
        );
}

fn build_backup_routing_policies(
    rows: &[BackupRoutingPolicyRow],
    secret_refs: &mut BTreeMap<Uuid, IndexerBackupSecretRef>,
) -> Vec<IndexerBackupRoutingPolicyItem> {
    let mut policies = BTreeMap::<String, IndexerBackupRoutingPolicyItem>::new();
    let mut parameter_keys = BTreeMap::<
        String,
        BTreeSet<(
            String,
            Option<Uuid>,
            Option<String>,
            Option<i32>,
            Option<bool>,
        )>,
    >::new();

    for row in rows {
        let entry = policies.entry(row.display_name.clone()).or_insert_with(|| {
            IndexerBackupRoutingPolicyItem {
                display_name: row.display_name.clone(),
                mode: row.mode.clone(),
                rate_limit_display_name: row.rate_limit_display_name.clone(),
                parameters: Vec::new(),
            }
        });
        if let Some(param_key) = row.param_key.clone() {
            if let (Some(secret_public_id), Some(secret_type)) =
                (row.secret_public_id, row.secret_type.clone())
            {
                secret_refs.insert(
                    secret_public_id,
                    IndexerBackupSecretRef {
                        secret_public_id,
                        secret_type,
                    },
                );
            }
            let key_set = parameter_keys.entry(row.display_name.clone()).or_default();
            key_set.insert((
                param_key,
                row.secret_public_id,
                row.value_plain.clone(),
                row.value_int,
                row.value_bool,
            ));
        }
        entry.rate_limit_display_name = entry
            .rate_limit_display_name
            .clone()
            .or_else(|| row.rate_limit_display_name.clone());
    }

    for (display_name, key_set) in parameter_keys {
        if let Some(policy) = policies.get_mut(&display_name) {
            policy.parameters = key_set
                .into_iter()
                .map(
                    |(param_key, secret_public_id, value_plain, value_int, value_bool)| {
                        IndexerBackupRoutingParameterItem {
                            param_key,
                            value_plain,
                            value_int,
                            value_bool,
                            secret_public_id,
                        }
                    },
                )
                .collect();
        }
    }

    policies.into_values().collect()
}

fn build_backup_indexer_instances(
    rows: &[BackupIndexerInstanceRow],
    secret_refs: &mut BTreeMap<Uuid, IndexerBackupSecretRef>,
) -> Vec<IndexerBackupIndexerInstanceItem> {
    let mut instances = BTreeMap::<String, IndexerBackupIndexerInstanceItem>::new();
    let mut media_domain_keys = BTreeMap::<String, BTreeSet<String>>::new();
    let mut tag_keys = BTreeMap::<String, BTreeSet<String>>::new();
    let mut fields = BTreeMap::<String, BTreeMap<String, IndexerBackupFieldItem>>::new();

    for row in rows {
        update_backup_instance_entry(&mut instances, row);
        insert_backup_media_domain(&mut media_domain_keys, row);
        insert_backup_tag(&mut tag_keys, row);
        insert_backup_field(&mut fields, secret_refs, row);
    }

    for (display_name, media_domain_set) in media_domain_keys {
        if let Some(instance) = instances.get_mut(&display_name) {
            instance.media_domain_keys = media_domain_set.into_iter().collect();
        }
    }
    for (display_name, tag_set) in tag_keys {
        if let Some(instance) = instances.get_mut(&display_name) {
            instance.tag_keys = tag_set.into_iter().collect();
        }
    }
    for (display_name, field_map) in fields {
        if let Some(instance) = instances.get_mut(&display_name) {
            instance.fields = field_map.into_values().collect();
        }
    }

    instances.into_values().collect()
}

fn update_backup_instance_entry(
    instances: &mut BTreeMap<String, IndexerBackupIndexerInstanceItem>,
    row: &BackupIndexerInstanceRow,
) {
    let entry = instances
        .entry(row.display_name.clone())
        .or_insert_with(|| IndexerBackupIndexerInstanceItem {
            upstream_slug: row.upstream_slug.clone(),
            display_name: row.display_name.clone(),
            instance_status: row.instance_status.clone(),
            rss_status: row.rss_status.clone(),
            automatic_search_status: row.automatic_search_status.clone(),
            interactive_search_status: row.interactive_search_status.clone(),
            priority: row.priority,
            trust_tier_key: row.trust_tier_key.clone(),
            routing_policy_display_name: row.routing_policy_display_name.clone(),
            connect_timeout_ms: row.connect_timeout_ms,
            read_timeout_ms: row.read_timeout_ms,
            max_parallel_requests: row.max_parallel_requests,
            rate_limit_display_name: row.rate_limit_display_name.clone(),
            rss_subscription_enabled: row.rss_subscription_enabled,
            rss_interval_seconds: row.rss_interval_seconds,
            media_domain_keys: Vec::new(),
            tag_keys: Vec::new(),
            fields: Vec::new(),
        });
    entry.routing_policy_display_name = entry
        .routing_policy_display_name
        .clone()
        .or_else(|| row.routing_policy_display_name.clone());
    entry.rate_limit_display_name = entry
        .rate_limit_display_name
        .clone()
        .or_else(|| row.rate_limit_display_name.clone());
    entry.rss_subscription_enabled = entry
        .rss_subscription_enabled
        .or(row.rss_subscription_enabled);
    entry.rss_interval_seconds = entry.rss_interval_seconds.or(row.rss_interval_seconds);
}

fn insert_backup_media_domain(
    media_domain_keys: &mut BTreeMap<String, BTreeSet<String>>,
    row: &BackupIndexerInstanceRow,
) {
    if let Some(media_domain_key) = row.media_domain_key.clone() {
        media_domain_keys
            .entry(row.display_name.clone())
            .or_default()
            .insert(media_domain_key);
    }
}

fn insert_backup_tag(
    tag_keys: &mut BTreeMap<String, BTreeSet<String>>,
    row: &BackupIndexerInstanceRow,
) {
    if let Some(tag_key) = row.tag_key.clone() {
        tag_keys
            .entry(row.display_name.clone())
            .or_default()
            .insert(tag_key);
    }
}

fn insert_backup_field(
    fields: &mut BTreeMap<String, BTreeMap<String, IndexerBackupFieldItem>>,
    secret_refs: &mut BTreeMap<Uuid, IndexerBackupSecretRef>,
    row: &BackupIndexerInstanceRow,
) {
    let Some(field_name) = row.field_name.clone() else {
        return;
    };

    if let (Some(secret_public_id), Some(secret_type)) =
        (row.secret_public_id, row.secret_type.clone())
    {
        secret_refs.insert(
            secret_public_id,
            IndexerBackupSecretRef {
                secret_public_id,
                secret_type,
            },
        );
    }

    fields
        .entry(row.display_name.clone())
        .or_default()
        .entry(field_name.clone())
        .or_insert_with(|| IndexerBackupFieldItem {
            field_name,
            field_type: row.field_type.clone().unwrap_or_default(),
            value_plain: row.value_plain.clone(),
            value_int: row.value_int,
            value_decimal: row.value_decimal.clone(),
            value_bool: row.value_bool,
            secret_public_id: row.secret_public_id,
        });
}

fn map_indexer_backup_error(
    _operation: &'static str,
    error: &DataError,
) -> IndexerBackupServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = indexer_backup_error_kind(code.as_deref());

    let mut service_error = IndexerBackupServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn indexer_backup_error_kind(detail: Option<&str>) -> IndexerBackupServiceErrorKind {
    match detail {
        Some("actor_missing" | "actor_not_found" | "actor_unverified" | "actor_unauthorized") => {
            IndexerBackupServiceErrorKind::Unauthorized
        }
        Some(
            "display_name_already_exists"
            | "tag_key_already_exists"
            | "duplicate_field_name"
            | "routing_policy_deleted",
        ) => IndexerBackupServiceErrorKind::Conflict,
        Some(
            "indexer_definition_not_found"
            | "routing_policy_not_found"
            | "rate_limit_policy_not_found"
            | "secret_not_found",
        ) => IndexerBackupServiceErrorKind::NotFound,
        Some("rate_limit_reference_missing" | "routing_policy_reference_missing") => {
            IndexerBackupServiceErrorKind::Invalid
        }
        _ => IndexerBackupServiceErrorKind::Storage,
    }
}

fn lookup_backup_reference(
    ids_by_name: &BTreeMap<String, Uuid>,
    display_name: &str,
    error_code: &'static str,
) -> Result<Uuid, IndexerBackupServiceError> {
    ids_by_name.get(display_name).copied().ok_or_else(|| {
        IndexerBackupServiceError::new(IndexerBackupServiceErrorKind::Invalid).with_code(error_code)
    })
}

fn map_indexer_backup_tag_error(error: &TagServiceError) -> IndexerBackupServiceError {
    let mut service_error = IndexerBackupServiceError::new(match error.kind() {
        TagServiceErrorKind::Invalid => IndexerBackupServiceErrorKind::Invalid,
        TagServiceErrorKind::NotFound => IndexerBackupServiceErrorKind::NotFound,
        TagServiceErrorKind::Conflict => IndexerBackupServiceErrorKind::Conflict,
        TagServiceErrorKind::Unauthorized => IndexerBackupServiceErrorKind::Unauthorized,
        TagServiceErrorKind::Storage => IndexerBackupServiceErrorKind::Storage,
    });
    service_error = service_error.with_code(error.code().unwrap_or("tag_backup_restore_failed"));
    if let Some(sqlstate) = error.sqlstate() {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn map_indexer_backup_rate_limit_error(
    error: &RateLimitPolicyServiceError,
) -> IndexerBackupServiceError {
    let mut service_error = IndexerBackupServiceError::new(match error.kind() {
        RateLimitPolicyServiceErrorKind::Invalid => IndexerBackupServiceErrorKind::Invalid,
        RateLimitPolicyServiceErrorKind::NotFound => IndexerBackupServiceErrorKind::NotFound,
        RateLimitPolicyServiceErrorKind::Conflict => IndexerBackupServiceErrorKind::Conflict,
        RateLimitPolicyServiceErrorKind::Unauthorized => {
            IndexerBackupServiceErrorKind::Unauthorized
        }
        RateLimitPolicyServiceErrorKind::Storage => IndexerBackupServiceErrorKind::Storage,
    });
    service_error =
        service_error.with_code(error.code().unwrap_or("rate_limit_backup_restore_failed"));
    if let Some(sqlstate) = error.sqlstate() {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn map_indexer_backup_routing_error(
    error: &RoutingPolicyServiceError,
) -> IndexerBackupServiceError {
    let mut service_error = IndexerBackupServiceError::new(match error.kind() {
        RoutingPolicyServiceErrorKind::Invalid => IndexerBackupServiceErrorKind::Invalid,
        RoutingPolicyServiceErrorKind::NotFound => IndexerBackupServiceErrorKind::NotFound,
        RoutingPolicyServiceErrorKind::Conflict => IndexerBackupServiceErrorKind::Conflict,
        RoutingPolicyServiceErrorKind::Unauthorized => IndexerBackupServiceErrorKind::Unauthorized,
        RoutingPolicyServiceErrorKind::Storage => IndexerBackupServiceErrorKind::Storage,
    });
    service_error =
        service_error.with_code(error.code().unwrap_or("routing_backup_restore_failed"));
    if let Some(sqlstate) = error.sqlstate() {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn map_indexer_backup_indexer_error(
    error: &IndexerInstanceServiceError,
) -> IndexerBackupServiceError {
    let mut service_error = IndexerBackupServiceError::new(match error.kind() {
        IndexerInstanceServiceErrorKind::Invalid => IndexerBackupServiceErrorKind::Invalid,
        IndexerInstanceServiceErrorKind::NotFound => IndexerBackupServiceErrorKind::NotFound,
        IndexerInstanceServiceErrorKind::Conflict => IndexerBackupServiceErrorKind::Conflict,
        IndexerInstanceServiceErrorKind::Unauthorized => {
            IndexerBackupServiceErrorKind::Unauthorized
        }
        IndexerInstanceServiceErrorKind::Storage => IndexerBackupServiceErrorKind::Storage,
    });
    service_error =
        service_error.with_code(error.code().unwrap_or("indexer_backup_restore_failed"));
    if let Some(sqlstate) = error.sqlstate() {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn map_indexer_backup_field_error(error: &IndexerInstanceFieldError) -> IndexerBackupServiceError {
    let mut service_error = IndexerBackupServiceError::new(match error.kind() {
        IndexerInstanceFieldErrorKind::Invalid => IndexerBackupServiceErrorKind::Invalid,
        IndexerInstanceFieldErrorKind::NotFound => IndexerBackupServiceErrorKind::NotFound,
        IndexerInstanceFieldErrorKind::Conflict => IndexerBackupServiceErrorKind::Conflict,
        IndexerInstanceFieldErrorKind::Unauthorized => IndexerBackupServiceErrorKind::Unauthorized,
        IndexerInstanceFieldErrorKind::Storage => IndexerBackupServiceErrorKind::Storage,
    });
    service_error = service_error.with_code(
        error
            .code()
            .unwrap_or("indexer_field_backup_restore_failed"),
    );
    if let Some(sqlstate) = error.sqlstate() {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn is_missing_secret_error(code: Option<&str>) -> bool {
    matches!(code, Some("secret_not_found"))
}

fn map_policy_error(_operation: &'static str, error: &DataError) -> PolicyServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = policy_error_kind(code.as_deref());

    let mut service_error = PolicyServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn policy_error_kind(detail: Option<&str>) -> PolicyServiceErrorKind {
    match detail {
        Some("policy_set_not_found" | "policy_rule_not_found") => PolicyServiceErrorKind::NotFound,
        Some("global_policy_set_exists" | "user_policy_set_exists" | "policy_set_deleted") => {
            PolicyServiceErrorKind::Conflict
        }
        Some("actor_missing" | "actor_not_found" | "actor_unauthorized") => {
            PolicyServiceErrorKind::Unauthorized
        }
        Some(
            "display_name_missing"
            | "display_name_empty"
            | "display_name_too_long"
            | "scope_missing"
            | "policy_set_missing"
            | "policy_set_ids_missing"
            | "policy_set_ids_empty"
            | "profile_policy_set_requires_link"
            | "policy_rule_ids_missing"
            | "policy_rule_ids_empty"
            | "policy_rule_missing"
            | "rationale_too_long"
            | "match_value_invalid"
            | "value_set_missing"
            | "value_set_not_allowed"
            | "value_set_too_large"
            | "value_set_item_invalid"
            | "value_set_duplicate"
            | "match_operator_invalid"
            | "rule_definition_missing"
            | "rule_action_missing"
            | "match_field_invalid"
            | "action_invalid",
        ) => PolicyServiceErrorKind::Invalid,
        _ => PolicyServiceErrorKind::Storage,
    }
}

fn map_category_mapping_error(
    _operation: &'static str,
    error: &DataError,
) -> CategoryMappingServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = category_mapping_error_kind(code.as_deref());

    let mut service_error = CategoryMappingServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn category_mapping_error_kind(detail: Option<&str>) -> CategoryMappingServiceErrorKind {
    match detail {
        Some(
            "mapping_not_found"
            | "indexer_definition_not_found"
            | "indexer_instance_not_found"
            | "indexer_instance_deleted"
            | "media_domain_not_found"
            | "torznab_category_not_found"
            | "torznab_instance_not_found"
            | "torznab_instance_deleted"
            | "unknown_key",
        ) => CategoryMappingServiceErrorKind::NotFound,
        Some("actor_missing" | "actor_not_found" | "actor_unauthorized") => {
            CategoryMappingServiceErrorKind::Unauthorized
        }
        Some(
            "tracker_category_missing"
            | "tracker_category_invalid"
            | "tracker_subcategory_invalid"
            | "torznab_category_missing"
            | "media_domain_missing"
            | "media_domain_key_invalid"
            | "indexer_slug_invalid"
            | "indexer_scope_conflict",
        ) => CategoryMappingServiceErrorKind::Invalid,
        _ => CategoryMappingServiceErrorKind::Storage,
    }
}

fn map_torznab_instance_error(
    _operation: &'static str,
    error: &DataError,
) -> TorznabInstanceServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = torznab_instance_error_kind(code.as_deref());

    let mut service_error = TorznabInstanceServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn torznab_instance_error_kind(detail: Option<&str>) -> TorznabInstanceServiceErrorKind {
    match detail {
        Some("torznab_instance_not_found" | "search_profile_not_found") => {
            TorznabInstanceServiceErrorKind::NotFound
        }
        Some(
            "display_name_already_exists" | "search_profile_deleted" | "torznab_instance_deleted",
        ) => TorznabInstanceServiceErrorKind::Conflict,
        Some(
            "actor_missing"
            | "actor_not_found"
            | "actor_unauthorized"
            | "search_profile_missing"
            | "display_name_missing"
            | "display_name_empty"
            | "display_name_too_long"
            | "torznab_instance_missing",
        ) => TorznabInstanceServiceErrorKind::Invalid,
        _ => TorznabInstanceServiceErrorKind::Storage,
    }
}

fn map_torznab_access_error(_operation: &'static str, error: &DataError) -> TorznabAccessError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = torznab_access_error_kind(code.as_deref());

    let mut service_error = TorznabAccessError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn torznab_access_error_kind(detail: Option<&str>) -> TorznabAccessErrorKind {
    match detail {
        Some("api_key_missing" | "api_key_invalid") => TorznabAccessErrorKind::Unauthorized,
        Some(
            "torznab_instance_missing"
            | "torznab_instance_not_found"
            | "torznab_instance_deleted"
            | "torznab_instance_disabled"
            | "canonical_source_missing"
            | "canonical_source_not_found"
            | "source_not_in_profile"
            | "canonical_not_found",
        ) => TorznabAccessErrorKind::NotFound,
        _ => TorznabAccessErrorKind::Storage,
    }
}

fn map_secret_error(_operation: &'static str, error: &DataError) -> SecretServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = secret_error_kind(code.as_deref());

    let mut service_error = SecretServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn secret_error_kind(detail: Option<&str>) -> SecretServiceErrorKind {
    match detail {
        Some("actor_missing" | "actor_not_found" | "actor_unauthorized") => {
            SecretServiceErrorKind::Unauthorized
        }
        Some("secret_not_found") => SecretServiceErrorKind::NotFound,
        Some("secret_type_missing" | "secret_value_missing" | "secret_missing") => {
            SecretServiceErrorKind::Invalid
        }
        _ => SecretServiceErrorKind::Storage,
    }
}

fn map_indexer_instance_error(
    _operation: &'static str,
    error: &DataError,
) -> IndexerInstanceServiceError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = indexer_instance_error_kind(code.as_deref());

    let mut service_error = IndexerInstanceServiceError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn indexer_instance_error_kind(detail: Option<&str>) -> IndexerInstanceServiceErrorKind {
    match detail {
        Some(
            "indexer_not_found"
            | "definition_not_found"
            | "routing_policy_not_found"
            | "tag_not_found"
            | "unknown_key",
        ) => IndexerInstanceServiceErrorKind::NotFound,
        Some(
            "display_name_already_exists"
            | "routing_policy_deleted"
            | "definition_deprecated"
            | "rss_enable_indexer_disabled",
        ) => IndexerInstanceServiceErrorKind::Conflict,
        Some(
            "actor_missing"
            | "actor_not_found"
            | "actor_unauthorized"
            | "indexer_missing"
            | "definition_missing"
            | "display_name_missing"
            | "display_name_empty"
            | "display_name_too_long"
            | "priority_out_of_range"
            | "unsupported_protocol"
            | "media_domain_key_invalid"
            | "tag_reference_missing"
            | "tag_key_invalid"
            | "invalid_tag_reference"
            | "indexer_deleted"
            | "reason_empty"
            | "reason_missing"
            | "reason_too_long"
            | "limit_out_of_range"
            | "rss_item_identifier_missing"
            | "item_guid_too_long"
            | "infohash_v1_invalid"
            | "infohash_v2_invalid"
            | "magnet_hash_invalid",
        ) => IndexerInstanceServiceErrorKind::Invalid,
        _ => IndexerInstanceServiceErrorKind::Storage,
    }
}

fn map_indexer_instance_field_error(
    _operation: &'static str,
    error: &DataError,
) -> IndexerInstanceFieldError {
    let code = error.database_detail().map(str::to_string);
    let sqlstate = error.database_code();
    let kind = indexer_instance_field_error_kind(code.as_deref());

    let mut service_error = IndexerInstanceFieldError::new(kind);
    if let Some(code) = code {
        service_error = service_error.with_code(code);
    }
    if let Some(sqlstate) = sqlstate {
        service_error = service_error.with_sqlstate(sqlstate);
    }
    service_error
}

fn indexer_instance_field_error_kind(detail: Option<&str>) -> IndexerInstanceFieldErrorKind {
    match detail {
        Some("indexer_not_found" | "indexer_deleted" | "field_not_found" | "secret_not_found") => {
            IndexerInstanceFieldErrorKind::NotFound
        }
        Some("field_type_mismatch" | "field_not_secret" | "field_requires_secret") => {
            IndexerInstanceFieldErrorKind::Conflict
        }
        Some(
            "value_type_mismatch"
            | "value_count_invalid"
            | "value_empty"
            | "value_too_long"
            | "value_too_short"
            | "value_too_small"
            | "value_too_large"
            | "value_regex_mismatch"
            | "value_not_allowed"
            | "value_required"
            | "field_name_missing"
            | "field_name_empty"
            | "field_name_too_long"
            | "indexer_missing"
            | "secret_missing",
        ) => IndexerInstanceFieldErrorKind::Invalid,
        Some("actor_missing" | "actor_not_found" | "actor_unauthorized") => {
            IndexerInstanceFieldErrorKind::Unauthorized
        }
        _ => IndexerInstanceFieldErrorKind::Storage,
    }
}

#[cfg(test)]
#[path = "indexers/tests/mod.rs"]
mod tests;
