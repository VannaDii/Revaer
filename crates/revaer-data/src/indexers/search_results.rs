//! Stored-procedure access for search result ingestion.
//!
//! # Design
//! - Encapsulates search result ingestion behind a typed wrapper.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

const SEARCH_RESULT_INGEST_CALL: &str = r"
    SELECT
        canonical_torrent_public_id,
        canonical_torrent_source_public_id,
        observation_created,
        durable_source_created,
        canonical_changed
    FROM search_result_ingest(
        search_request_public_id_input => $1,
        indexer_instance_public_id_input => $2,
        source_guid_input => $3,
        details_url_input => $4,
        download_url_input => $5,
        magnet_uri_input => $6,
        title_raw_input => $7,
        size_bytes_input => $8,
        infohash_v1_input => $9,
        infohash_v2_input => $10,
        magnet_hash_input => $11,
        seeders_input => $12,
        leechers_input => $13,
        published_at_input => $14,
        uploader_input => $15,
        observed_at_input => $16,
        attr_keys_input => $17::observation_attr_key[],
        attr_types_input => $18::attr_value_type[],
        attr_value_text_input => $19,
        attr_value_int_input => $20,
        attr_value_bigint_input => $21,
        attr_value_numeric_input => $22::numeric[],
        attr_value_bool_input => $23,
        attr_value_uuid_input => $24
    )
";

/// Output from search result ingestion.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SearchResultIngestRow {
    /// Canonical torrent public id.
    pub canonical_torrent_public_id: Uuid,
    /// Canonical torrent source public id.
    pub canonical_torrent_source_public_id: Uuid,
    /// Whether an observation row was created.
    pub observation_created: bool,
    /// Whether a durable source row was created.
    pub durable_source_created: bool,
    /// Whether the canonical torrent changed.
    pub canonical_changed: bool,
}

/// Input payload for search result ingestion.
#[derive(Debug, Clone)]
pub struct SearchResultIngestInput<'a> {
    /// Search request public id.
    pub search_request_public_id: Uuid,
    /// Indexer instance public id.
    pub indexer_instance_public_id: Uuid,
    /// Optional source guid.
    pub source_guid: Option<&'a str>,
    /// Optional details URL.
    pub details_url: Option<&'a str>,
    /// Optional download URL.
    pub download_url: Option<&'a str>,
    /// Optional magnet URI.
    pub magnet_uri: Option<&'a str>,
    /// Raw title string.
    pub title_raw: &'a str,
    /// Optional size in bytes.
    pub size_bytes: Option<i64>,
    /// Optional infohash v1.
    pub infohash_v1: Option<&'a str>,
    /// Optional infohash v2.
    pub infohash_v2: Option<&'a str>,
    /// Optional magnet hash.
    pub magnet_hash: Option<&'a str>,
    /// Optional seeders count.
    pub seeders: Option<i32>,
    /// Optional leechers count.
    pub leechers: Option<i32>,
    /// Optional published timestamp.
    pub published_at: Option<DateTime<Utc>>,
    /// Optional uploader label.
    pub uploader: Option<&'a str>,
    /// Observed timestamp.
    pub observed_at: DateTime<Utc>,
    /// Observation attribute keys.
    pub attr_keys: Option<&'a [String]>,
    /// Observation attribute types.
    pub attr_types: Option<&'a [String]>,
    /// Observation attribute text values.
    pub attr_value_text: Option<&'a [Option<String>]>,
    /// Observation attribute int values.
    pub attr_value_int: Option<&'a [Option<i32>]>,
    /// Observation attribute bigint values.
    pub attr_value_bigint: Option<&'a [Option<i64>]>,
    /// Observation attribute numeric values (string encoded).
    pub attr_value_numeric: Option<&'a [Option<String>]>,
    /// Observation attribute boolean values.
    pub attr_value_bool: Option<&'a [Option<bool>]>,
    /// Observation attribute UUID values.
    pub attr_value_uuid: Option<&'a [Option<Uuid>]>,
}

/// Ingest a search result observation.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn search_result_ingest(
    pool: &PgPool,
    input: &SearchResultIngestInput<'_>,
) -> Result<SearchResultIngestRow> {
    sqlx::query_as(SEARCH_RESULT_INGEST_CALL)
        .bind(input.search_request_public_id)
        .bind(input.indexer_instance_public_id)
        .bind(input.source_guid)
        .bind(input.details_url)
        .bind(input.download_url)
        .bind(input.magnet_uri)
        .bind(input.title_raw)
        .bind(input.size_bytes)
        .bind(input.infohash_v1)
        .bind(input.infohash_v2)
        .bind(input.magnet_hash)
        .bind(input.seeders)
        .bind(input.leechers)
        .bind(input.published_at)
        .bind(input.uploader)
        .bind(input.observed_at)
        .bind(input.attr_keys)
        .bind(input.attr_types)
        .bind(input.attr_value_text)
        .bind(input.attr_value_int)
        .bind(input.attr_value_bigint)
        .bind(input.attr_value_numeric)
        .bind(input.attr_value_bool)
        .bind(input.attr_value_uuid)
        .fetch_one(pool)
        .await
        .map_err(try_op("search result ingest"))
}

#[cfg(test)]
#[path = "search_results/tests.rs"]
mod tests;
