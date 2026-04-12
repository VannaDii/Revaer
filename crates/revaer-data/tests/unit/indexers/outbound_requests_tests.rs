use super::*;
use crate::DataError;
async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("indexer tests").await
}
#[tokio::test]
async fn outbound_request_log_write_requires_instance() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let now = test_db.now();

    let input = OutboundRequestLogInput {
        indexer_instance_public_id: Uuid::new_v4(),
        routing_policy_public_id: None,
        search_request_public_id: None,
        request_type: "caps",
        correlation_id: Uuid::new_v4(),
        retry_seq: 0,
        started_at: now,
        finished_at: now,
        outcome: "failure",
        via_mitigation: "none",
        rate_limit_denied_scope: None,
        error_class: None,
        http_status: None,
        latency_ms: None,
        parse_ok: None,
        result_count: None,
        cf_detected: None,
        page_number: None,
        page_cursor_key: None,
    };

    let err = outbound_request_log_write(pool, &input).await.unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("indexer_instance_not_found"));
    Ok(())
}
