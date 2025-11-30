//! Documentation endpoints.

use std::sync::Arc;

use axum::{Json, extract::State};
use serde_json::Value;

use crate::state::ApiState;

pub(crate) async fn openapi_document_handler(State(state): State<Arc<ApiState>>) -> Json<Value> {
    Json((*state.openapi_document).clone())
}
