//! Indexer definition catalog endpoints.
//!
//! # Design
//! - Delegate catalog reads to the injected indexer facade.
//! - Surface stable RFC9457 errors with context fields for diagnostics.
//! - Keep messages constant and avoid leaking input values in responses.

use std::sync::Arc;

use crate::app::indexers::{IndexerDefinitionServiceError, IndexerDefinitionServiceErrorKind};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::models::{
    CardigannDefinitionImportRequest, CardigannDefinitionImportResponse,
    IndexerDefinitionListResponse,
};
use axum::{Json, extract::State, http::StatusCode};

pub(crate) async fn list_indexer_definitions(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<IndexerDefinitionListResponse>, ApiError> {
    const INDEXER_DEFINITION_LIST_FAILED: &str = "failed to list indexer definitions";
    let definitions = state
        .indexers
        .indexer_definition_list(SYSTEM_ACTOR_PUBLIC_ID)
        .await
        .map_err(|err| {
            map_indexer_definition_error(
                "indexer_definition_list",
                INDEXER_DEFINITION_LIST_FAILED,
                &err,
            )
        })?;

    Ok(Json(IndexerDefinitionListResponse { definitions }))
}

pub(crate) async fn import_cardigann_definition(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CardigannDefinitionImportRequest>,
) -> Result<(StatusCode, Json<CardigannDefinitionImportResponse>), ApiError> {
    const INDEXER_DEFINITION_IMPORT_FAILED: &str = "failed to import Cardigann definition";
    let response = state
        .indexers
        .indexer_definition_import_cardigann(
            SYSTEM_ACTOR_PUBLIC_ID,
            request.yaml_payload.trim(),
            request.is_deprecated,
        )
        .await
        .map_err(|err| {
            map_indexer_definition_error(
                "indexer_definition_import_cardigann",
                INDEXER_DEFINITION_IMPORT_FAILED,
                &err,
            )
        })?;

    Ok((StatusCode::CREATED, Json(response)))
}

fn map_indexer_definition_error(
    operation: &'static str,
    detail: &'static str,
    err: &IndexerDefinitionServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        IndexerDefinitionServiceErrorKind::Invalid => ApiError::bad_request(detail),
        IndexerDefinitionServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        IndexerDefinitionServiceErrorKind::Storage => ApiError::internal(detail),
    };

    api_error = api_error.with_context_field("operation", operation);
    if let Some(code) = err.code() {
        api_error = api_error.with_context_field("error_code", code);
    }
    if let Some(sqlstate) = err.sqlstate() {
        api_error = api_error.with_context_field("sqlstate", sqlstate);
    }
    api_error
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::indexers::{
        CategoryMappingServiceError, CategoryMappingServiceErrorKind, IndexerCfStateResetParams,
        IndexerDefinitionServiceError, IndexerDefinitionServiceErrorKind, IndexerFacade,
        IndexerInstanceFieldError, IndexerInstanceFieldErrorKind, IndexerInstanceFieldValueParams,
        IndexerInstanceServiceError, IndexerInstanceServiceErrorKind,
        IndexerInstanceTestFinalizeParams, IndexerInstanceUpdateParams,
        RateLimitPolicyServiceError, RateLimitPolicyServiceErrorKind, RoutingPolicyServiceError,
        RoutingPolicyServiceErrorKind, SearchProfileServiceError, SearchProfileServiceErrorKind,
        SecretServiceError, SecretServiceErrorKind, TagServiceError, TagServiceErrorKind,
        TorznabInstanceCredentials, TorznabInstanceServiceError, TorznabInstanceServiceErrorKind,
    };
    use crate::http::handlers::indexers::test_support::{indexer_test_state, parse_problem};
    use crate::models::{
        CardigannDefinitionImportResponse, IndexerCfStateResponse, IndexerDefinitionResponse,
        IndexerInstanceTestFinalizeResponse, IndexerInstanceTestPrepareResponse,
    };
    use async_trait::async_trait;
    use axum::response::IntoResponse;
    use chrono::{TimeZone, Utc};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    #[derive(Default)]
    struct RecordingIndexers {
        calls: Mutex<Vec<Uuid>>,
        result:
            Mutex<Option<Result<Vec<IndexerDefinitionResponse>, IndexerDefinitionServiceError>>>,
        import_result:
            Mutex<Option<Result<CardigannDefinitionImportResponse, IndexerDefinitionServiceError>>>,
    }

    impl RecordingIndexers {
        fn with_result(
            result: Result<Vec<IndexerDefinitionResponse>, IndexerDefinitionServiceError>,
        ) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                result: Mutex::new(Some(result)),
                import_result: Mutex::new(None),
            }
        }

        fn with_import_result(
            result: Result<CardigannDefinitionImportResponse, IndexerDefinitionServiceError>,
        ) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                result: Mutex::new(None),
                import_result: Mutex::new(Some(result)),
            }
        }

        fn take_calls(&self) -> Vec<Uuid> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .drain(..)
                .collect()
        }

        fn take_result(
            &self,
        ) -> Result<Vec<IndexerDefinitionResponse>, IndexerDefinitionServiceError> {
            self.result
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .unwrap_or_else(|| {
                    Err(IndexerDefinitionServiceError::new(
                        IndexerDefinitionServiceErrorKind::Storage,
                    ))
                })
        }

        fn take_import_result(
            &self,
        ) -> Result<CardigannDefinitionImportResponse, IndexerDefinitionServiceError> {
            self.import_result
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .unwrap_or_else(|| {
                    Err(IndexerDefinitionServiceError::new(
                        IndexerDefinitionServiceErrorKind::Storage,
                    ))
                })
        }
    }

    #[async_trait]
    impl IndexerFacade for RecordingIndexers {
        async fn indexer_definition_list(
            &self,
            actor_user_public_id: Uuid,
        ) -> Result<Vec<IndexerDefinitionResponse>, IndexerDefinitionServiceError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(actor_user_public_id);
            self.take_result()
        }

        async fn indexer_definition_import_cardigann(
            &self,
            actor_user_public_id: Uuid,
            _yaml_payload: &str,
            _is_deprecated: Option<bool>,
        ) -> Result<CardigannDefinitionImportResponse, IndexerDefinitionServiceError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(actor_user_public_id);
            self.take_import_result()
        }

        async fn tag_create(
            &self,
            _actor_user_public_id: Uuid,
            _tag_key: &str,
            _display_name: &str,
        ) -> Result<Uuid, TagServiceError> {
            Err(TagServiceError::new(TagServiceErrorKind::Storage))
        }

        async fn tag_update(
            &self,
            _actor_user_public_id: Uuid,
            _tag_public_id: Option<Uuid>,
            _tag_key: Option<&str>,
            _display_name: &str,
        ) -> Result<Uuid, TagServiceError> {
            Err(TagServiceError::new(TagServiceErrorKind::Storage))
        }

        async fn tag_delete(
            &self,
            _actor_user_public_id: Uuid,
            _tag_public_id: Option<Uuid>,
            _tag_key: Option<&str>,
        ) -> Result<(), TagServiceError> {
            Err(TagServiceError::new(TagServiceErrorKind::Storage))
        }

        async fn routing_policy_create(
            &self,
            _actor_user_public_id: Uuid,
            _display_name: &str,
            _mode: &str,
        ) -> Result<Uuid, RoutingPolicyServiceError> {
            Err(RoutingPolicyServiceError::new(
                RoutingPolicyServiceErrorKind::Storage,
            ))
        }

        async fn routing_policy_set_param(
            &self,
            _actor_user_public_id: Uuid,
            _routing_policy_public_id: Uuid,
            _param_key: &str,
            _value_plain: Option<&str>,
            _value_int: Option<i32>,
            _value_bool: Option<bool>,
        ) -> Result<(), RoutingPolicyServiceError> {
            Err(RoutingPolicyServiceError::new(
                RoutingPolicyServiceErrorKind::Storage,
            ))
        }

        async fn routing_policy_bind_secret(
            &self,
            _actor_user_public_id: Uuid,
            _routing_policy_public_id: Uuid,
            _param_key: &str,
            _secret_public_id: Uuid,
        ) -> Result<(), RoutingPolicyServiceError> {
            Err(RoutingPolicyServiceError::new(
                RoutingPolicyServiceErrorKind::Storage,
            ))
        }

        async fn rate_limit_policy_create(
            &self,
            _actor_user_public_id: Uuid,
            _display_name: &str,
            _rpm: i32,
            _burst: i32,
            _concurrent: i32,
        ) -> Result<Uuid, RateLimitPolicyServiceError> {
            Err(RateLimitPolicyServiceError::new(
                RateLimitPolicyServiceErrorKind::Storage,
            ))
        }

        async fn rate_limit_policy_update(
            &self,
            _actor_user_public_id: Uuid,
            _rate_limit_policy_public_id: Uuid,
            _display_name: Option<&str>,
            _rpm: Option<i32>,
            _burst: Option<i32>,
            _concurrent: Option<i32>,
        ) -> Result<(), RateLimitPolicyServiceError> {
            Err(RateLimitPolicyServiceError::new(
                RateLimitPolicyServiceErrorKind::Storage,
            ))
        }

        async fn rate_limit_policy_soft_delete(
            &self,
            _actor_user_public_id: Uuid,
            _rate_limit_policy_public_id: Uuid,
        ) -> Result<(), RateLimitPolicyServiceError> {
            Err(RateLimitPolicyServiceError::new(
                RateLimitPolicyServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_set_rate_limit_policy(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
            _rate_limit_policy_public_id: Option<Uuid>,
        ) -> Result<(), RateLimitPolicyServiceError> {
            Err(RateLimitPolicyServiceError::new(
                RateLimitPolicyServiceErrorKind::Storage,
            ))
        }

        async fn routing_policy_set_rate_limit_policy(
            &self,
            _actor_user_public_id: Uuid,
            _routing_policy_public_id: Uuid,
            _rate_limit_policy_public_id: Option<Uuid>,
        ) -> Result<(), RateLimitPolicyServiceError> {
            Err(RateLimitPolicyServiceError::new(
                RateLimitPolicyServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_create(
            &self,
            _actor_user_public_id: Uuid,
            _display_name: &str,
            _is_default: Option<bool>,
            _page_size: Option<i32>,
            _default_media_domain_key: Option<&str>,
            _user_public_id: Option<Uuid>,
        ) -> Result<Uuid, SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_update(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _display_name: Option<&str>,
            _page_size: Option<i32>,
        ) -> Result<Uuid, SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_set_default(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _page_size: Option<i32>,
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_set_default_domain(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _default_media_domain_key: Option<&str>,
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_set_domain_allowlist(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _media_domain_keys: &[String],
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_add_policy_set(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _policy_set_public_id: Uuid,
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_remove_policy_set(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _policy_set_public_id: Uuid,
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_indexer_allow(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _indexer_instance_public_ids: &[Uuid],
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_indexer_block(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _indexer_instance_public_ids: &[Uuid],
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_tag_allow(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _tag_public_ids: Option<&[Uuid]>,
            _tag_keys: Option<&[String]>,
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_tag_block(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _tag_public_ids: Option<&[Uuid]>,
            _tag_keys: Option<&[String]>,
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_tag_prefer(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _tag_public_ids: Option<&[Uuid]>,
            _tag_keys: Option<&[String]>,
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn tracker_category_mapping_upsert(
            &self,
            _params: crate::app::indexers::TrackerCategoryMappingUpsertParams<'_>,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(CategoryMappingServiceError::new(
                CategoryMappingServiceErrorKind::Storage,
            ))
        }

        async fn tracker_category_mapping_delete(
            &self,
            _params: crate::app::indexers::TrackerCategoryMappingDeleteParams<'_>,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(CategoryMappingServiceError::new(
                CategoryMappingServiceErrorKind::Storage,
            ))
        }

        async fn media_domain_mapping_upsert(
            &self,
            _actor_user_public_id: Uuid,
            _media_domain_key: &str,
            _torznab_cat_id: i32,
            _is_primary: Option<bool>,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(CategoryMappingServiceError::new(
                CategoryMappingServiceErrorKind::Storage,
            ))
        }

        async fn media_domain_mapping_delete(
            &self,
            _actor_user_public_id: Uuid,
            _media_domain_key: &str,
            _torznab_cat_id: i32,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(CategoryMappingServiceError::new(
                CategoryMappingServiceErrorKind::Storage,
            ))
        }

        async fn torznab_instance_create(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _display_name: &str,
        ) -> Result<TorznabInstanceCredentials, TorznabInstanceServiceError> {
            Err(TorznabInstanceServiceError::new(
                TorznabInstanceServiceErrorKind::Storage,
            ))
        }

        async fn torznab_instance_rotate_key(
            &self,
            _actor_user_public_id: Uuid,
            _torznab_instance_public_id: Uuid,
        ) -> Result<TorznabInstanceCredentials, TorznabInstanceServiceError> {
            Err(TorznabInstanceServiceError::new(
                TorznabInstanceServiceErrorKind::Storage,
            ))
        }

        async fn torznab_instance_enable_disable(
            &self,
            _actor_user_public_id: Uuid,
            _torznab_instance_public_id: Uuid,
            _is_enabled: bool,
        ) -> Result<(), TorznabInstanceServiceError> {
            Err(TorznabInstanceServiceError::new(
                TorznabInstanceServiceErrorKind::Storage,
            ))
        }

        async fn torznab_instance_soft_delete(
            &self,
            _actor_user_public_id: Uuid,
            _torznab_instance_public_id: Uuid,
        ) -> Result<(), TorznabInstanceServiceError> {
            Err(TorznabInstanceServiceError::new(
                TorznabInstanceServiceErrorKind::Storage,
            ))
        }

        async fn secret_create(
            &self,
            _actor_user_public_id: Uuid,
            _secret_type: &str,
            _secret_value: &str,
        ) -> Result<Uuid, SecretServiceError> {
            Err(SecretServiceError::new(SecretServiceErrorKind::Storage))
        }

        async fn secret_rotate(
            &self,
            _actor_user_public_id: Uuid,
            _secret_public_id: Uuid,
            _secret_value: &str,
        ) -> Result<Uuid, SecretServiceError> {
            Err(SecretServiceError::new(SecretServiceErrorKind::Storage))
        }

        async fn secret_revoke(
            &self,
            _actor_user_public_id: Uuid,
            _secret_public_id: Uuid,
        ) -> Result<(), SecretServiceError> {
            Err(SecretServiceError::new(SecretServiceErrorKind::Storage))
        }

        async fn indexer_cf_state_reset(
            &self,
            _params: IndexerCfStateResetParams<'_>,
        ) -> Result<(), IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_cf_state_get(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
        ) -> Result<IndexerCfStateResponse, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_create(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_definition_upstream_slug: &str,
            _display_name: &str,
            _priority: Option<i32>,
            _trust_tier_key: Option<&str>,
            _routing_policy_public_id: Option<Uuid>,
        ) -> Result<Uuid, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_update(
            &self,
            _params: IndexerInstanceUpdateParams<'_>,
        ) -> Result<Uuid, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_set_media_domains(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
            _media_domain_keys: &[String],
        ) -> Result<(), IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_set_tags(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
            _tag_public_ids: Option<&[Uuid]>,
            _tag_keys: Option<&[String]>,
        ) -> Result<(), IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_field_set_value(
            &self,
            _params: IndexerInstanceFieldValueParams<'_>,
        ) -> Result<(), IndexerInstanceFieldError> {
            Err(IndexerInstanceFieldError::new(
                IndexerInstanceFieldErrorKind::Storage,
            ))
        }

        async fn indexer_instance_field_bind_secret(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
            _field_name: &str,
            _secret_public_id: Uuid,
        ) -> Result<(), IndexerInstanceFieldError> {
            Err(IndexerInstanceFieldError::new(
                IndexerInstanceFieldErrorKind::Storage,
            ))
        }

        async fn indexer_instance_test_prepare(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
        ) -> Result<IndexerInstanceTestPrepareResponse, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_test_finalize(
            &self,
            _params: IndexerInstanceTestFinalizeParams<'_>,
        ) -> Result<IndexerInstanceTestFinalizeResponse, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }
    }

    #[tokio::test]
    async fn list_indexer_definitions_returns_payload() -> Result<(), ApiError> {
        let definition = IndexerDefinitionResponse {
            upstream_source: "prowlarr_indexers".to_string(),
            upstream_slug: "alpha".to_string(),
            display_name: "Alpha".to_string(),
            protocol: "torrent".to_string(),
            engine: "torznab".to_string(),
            schema_version: 1,
            definition_hash: "a".repeat(64),
            is_deprecated: false,
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap(),
        };
        let indexers = Arc::new(RecordingIndexers::with_result(Ok(vec![definition.clone()])));
        let state = indexer_test_state(indexers.clone())?;

        let Json(payload) = list_indexer_definitions(State(state)).await?;
        assert_eq!(payload.definitions, vec![definition]);

        let calls = indexers.take_calls();
        assert_eq!(calls, vec![SYSTEM_ACTOR_PUBLIC_ID]);
        Ok(())
    }

    #[tokio::test]
    async fn list_indexer_definitions_maps_unauthorized() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::with_result(Err(
            IndexerDefinitionServiceError::new(IndexerDefinitionServiceErrorKind::Unauthorized)
                .with_code("actor_missing"),
        )));
        let state = indexer_test_state(indexers)?;

        let err = list_indexer_definitions(State(state)).await.err().unwrap();
        let response = err.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
        let problem = parse_problem(response).await;
        let context = problem.context.unwrap_or_default();
        assert!(context.iter().any(|field| field.name == "operation"));
        Ok(())
    }

    #[tokio::test]
    async fn import_cardigann_definition_returns_payload() -> Result<(), ApiError> {
        let definition = IndexerDefinitionResponse {
            upstream_source: "cardigann".to_string(),
            upstream_slug: "example".to_string(),
            display_name: "Example".to_string(),
            protocol: "torrent".to_string(),
            engine: "cardigann".to_string(),
            schema_version: 1,
            definition_hash: "b".repeat(64),
            is_deprecated: false,
            created_at: Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
        };
        let indexers = Arc::new(RecordingIndexers::with_import_result(Ok(
            CardigannDefinitionImportResponse {
                definition: definition.clone(),
                field_count: 2,
                option_count: 3,
            },
        )));
        let state = indexer_test_state(indexers.clone())?;
        let request = CardigannDefinitionImportRequest {
            yaml_payload: "id: example\nname: Example\nsettings: []".to_string(),
            is_deprecated: Some(false),
        };

        let (status, Json(payload)) =
            import_cardigann_definition(State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(payload.definition, definition);
        assert_eq!(payload.field_count, 2);

        let calls = indexers.take_calls();
        assert_eq!(calls, vec![SYSTEM_ACTOR_PUBLIC_ID]);
        Ok(())
    }

    #[tokio::test]
    async fn import_cardigann_definition_maps_invalid() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::with_import_result(Err(
            IndexerDefinitionServiceError::new(IndexerDefinitionServiceErrorKind::Invalid)
                .with_code("cardigann_yaml_invalid"),
        )));
        let state = indexer_test_state(indexers)?;
        let request = CardigannDefinitionImportRequest {
            yaml_payload: "not-yaml".to_string(),
            is_deprecated: Some(false),
        };

        let err = import_cardigann_definition(State(state), Json(request))
            .await
            .expect_err("expected invalid request error");
        let response = err.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        let context = problem.context.unwrap_or_default();
        assert!(context.iter().any(|field| field.name == "error_code"));
        Ok(())
    }
}
