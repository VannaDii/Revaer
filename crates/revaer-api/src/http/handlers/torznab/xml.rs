//! XML helpers for Torznab responses.
//!
//! # Design
//! - Keep XML generation deterministic and allocation-safe.
//! - Escape all attribute values to avoid malformed XML.
//! - Limit output to Torznab v1 fields required by the ERD.

use crate::app::indexers::TorznabCategory;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::checked_string_capacity;
use chrono::{DateTime, Utc};
use std::fmt::Write as _;
use uuid::Uuid;

const TORZNAB_XML_HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";
const TORZNAB_XMLNS: &str = "http://torznab.com/schemas/2015/feed";
const OTHER_CATEGORY_ID: i32 = 8000;

/// Flattened Torznab feed item used for XML rendering.
#[derive(Debug, Clone)]
pub(super) struct TorznabFeedItem {
    /// Canonical source identifier used as guid.
    pub guid: Uuid,
    /// Display title for the release.
    pub title: String,
    /// Optional size in bytes.
    pub size_bytes: Option<i64>,
    /// Optional publication timestamp.
    pub published_at: Option<DateTime<Utc>>,
    /// Category IDs to emit.
    pub categories: Vec<i32>,
    /// Seeder count.
    pub seeders: i32,
    /// Leecher count.
    pub leechers: i32,
    /// Download volume factor (0 freeleech, 1 default).
    pub download_volume_factor: i32,
    /// Optional infohash value for torznab:attr.
    pub infohash: Option<String>,
    /// Download endpoint URL.
    pub download_link: String,
}

/// Build a Torznab caps XML response.
pub(super) fn build_caps_response(
    instance_display_name: &str,
    categories: &[TorznabCategory],
) -> Result<String, ApiError> {
    let title = escape_xml(instance_display_name)?;
    let required_capacity = caps_capacity_bytes(&title, categories);
    let mut xml = checked_string_capacity(required_capacity)?;
    xml.push_str(TORZNAB_XML_HEADER);
    xml.push_str("<caps>");
    xml.push_str("<server version=\"1.0\" title=\"");
    xml.push_str(&title);
    xml.push_str("\"/>");
    xml.push_str("<limits default=\"50\" max=\"200\"/>");
    xml.push_str("<searching>");
    xml.push_str("<search available=\"yes\" supportedParams=\"q,cat\"/>");
    xml.push_str(
        "<tv-search available=\"yes\" supportedParams=\"q,season,ep,imdbid,tvdbid,tmdbid,cat\"/>",
    );
    xml.push_str("<movie-search available=\"yes\" supportedParams=\"q,imdbid,tmdbid,cat\"/>");
    xml.push_str("</searching>");
    xml.push_str("<categories>");
    for category in categories {
        xml.push_str("<category id=\"");
        push_display(&mut xml, category.torznab_cat_id)?;
        xml.push_str("\" name=\"");
        xml.push_str(&escape_xml(&category.name)?);
        xml.push_str("\"/>");
    }
    xml.push_str("</categories>");
    xml.push_str("</caps>");
    Ok(xml)
}

/// Build an empty Torznab RSS response for invalid requests.
pub(super) fn build_empty_search_response() -> String {
    let mut xml = String::with_capacity(256);
    xml.push_str(TORZNAB_XML_HEADER);
    xml.push_str("<rss version=\"2.0\" xmlns:torznab=\"");
    xml.push_str(TORZNAB_XMLNS);
    xml.push_str("\">");
    xml.push_str("<channel>");
    xml.push_str("<title>Revaer Torznab</title>");
    xml.push_str("<description>Revaer Torznab</description>");
    xml.push_str("<torznab:response offset=\"0\" total=\"0\"/>");
    xml.push_str("</channel>");
    xml.push_str("</rss>");
    xml
}

/// Build a Torznab search RSS response with items.
pub(super) fn build_search_response(
    items: &[TorznabFeedItem],
    offset: i32,
    total: i32,
) -> Result<String, ApiError> {
    let mut xml = checked_string_capacity(search_capacity_bytes(items))?;
    xml.push_str(TORZNAB_XML_HEADER);
    xml.push_str("<rss version=\"2.0\" xmlns:torznab=\"");
    xml.push_str(TORZNAB_XMLNS);
    xml.push_str("\">");
    xml.push_str("<channel>");
    xml.push_str("<title>Revaer Torznab</title>");
    xml.push_str("<description>Revaer Torznab</description>");
    xml.push_str("<torznab:response offset=\"");
    push_display(&mut xml, offset)?;
    xml.push_str("\" total=\"");
    push_display(&mut xml, total)?;
    xml.push_str("\"/>");

    for item in items {
        xml.push_str("<item>");
        xml.push_str("<title>");
        xml.push_str(&escape_xml(&item.title)?);
        xml.push_str("</title>");
        xml.push_str("<guid isPermaLink=\"false\">");
        xml.push_str(&item.guid.to_string());
        xml.push_str("</guid>");
        xml.push_str("<link>");
        xml.push_str(&escape_xml(&item.download_link)?);
        xml.push_str("</link>");
        xml.push_str("<comments>");
        xml.push_str(&escape_xml(&item.download_link)?);
        xml.push_str("</comments>");
        if let Some(size_bytes) = item.size_bytes {
            xml.push_str("<size>");
            push_display(&mut xml, size_bytes)?;
            xml.push_str("</size>");
        }
        if let Some(published_at) = item.published_at {
            xml.push_str("<pubDate>");
            xml.push_str(&escape_xml(&published_at.to_rfc2822())?);
            xml.push_str("</pubDate>");
        }

        let categories = if item.categories.is_empty() {
            vec![OTHER_CATEGORY_ID]
        } else {
            item.categories.clone()
        };
        for category in categories {
            xml.push_str("<category>");
            push_display(&mut xml, category)?;
            xml.push_str("</category>");
        }

        xml.push_str("<torznab:attr name=\"seeders\" value=\"");
        push_display(&mut xml, item.seeders)?;
        xml.push_str("\"/>");
        xml.push_str("<torznab:attr name=\"leechers\" value=\"");
        push_display(&mut xml, item.leechers)?;
        xml.push_str("\"/>");
        xml.push_str("<torznab:attr name=\"peers\" value=\"");
        push_display(&mut xml, item.seeders.saturating_add(item.leechers))?;
        xml.push_str("\"/>");
        xml.push_str("<torznab:attr name=\"downloadvolumefactor\" value=\"");
        push_display(&mut xml, item.download_volume_factor)?;
        xml.push_str("\"/>");
        xml.push_str("<torznab:attr name=\"uploadvolumefactor\" value=\"1\"/>");
        if let Some(infohash) = &item.infohash {
            xml.push_str("<torznab:attr name=\"infohash\" value=\"");
            xml.push_str(&escape_xml(infohash)?);
            xml.push_str("\"/>");
        }
        xml.push_str("</item>");
    }

    xml.push_str("</channel>");
    xml.push_str("</rss>");
    Ok(xml)
}

fn push_display(xml: &mut String, value: impl std::fmt::Display) -> Result<(), ApiError> {
    write!(xml, "{value}").map_err(|_| ApiError::internal("failed to build torznab response"))
}

fn escape_xml(value: &str) -> Result<String, ApiError> {
    let capacity = escaped_xml_len(value);
    let mut escaped = checked_string_capacity(capacity)?;
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    Ok(escaped)
}

fn caps_capacity_bytes(title: &str, categories: &[TorznabCategory]) -> usize {
    let mut required = 256usize;
    required = required.saturating_add(title.len());
    for category in categories {
        required = required.saturating_add(escaped_xml_len(&category.name));
        required = required.saturating_add(64);
    }
    required
}

fn escaped_xml_len(value: &str) -> usize {
    value.chars().fold(0usize, |acc, ch| {
        let added = match ch {
            '&' | '\'' => 5,
            '<' | '>' => 4,
            '"' => 6,
            _ => ch.len_utf8(),
        };
        acc.saturating_add(added)
    })
}

fn search_capacity_bytes(items: &[TorznabFeedItem]) -> usize {
    let mut required = 512usize;
    for item in items {
        required = required.saturating_add(escaped_xml_len(&item.title));
        required = required.saturating_add(escaped_xml_len(&item.download_link) * 2);
        required = required.saturating_add(escaped_xml_len(
            &item
                .published_at
                .map_or_else(String::new, |value| value.to_rfc2822()),
        ));
        required = required.saturating_add(item.infohash.as_ref().map_or(0, String::len));
        required = required.saturating_add(512);
    }
    required
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_xml_rewrites_reserved_characters() {
        let input = "A&B <C> \"D\" 'E'";
        let escaped = escape_xml(input).expect("escape should succeed");
        assert_eq!(escaped, "A&amp;B &lt;C&gt; &quot;D&quot; &#39;E&#39;");
    }

    #[test]
    fn build_caps_response_includes_categories() {
        let categories = vec![TorznabCategory {
            torznab_cat_id: 2000,
            name: "Movies".to_string(),
        }];
        let xml =
            build_caps_response("Revaer & Co", &categories).expect("caps response should build");
        assert!(xml.contains("Revaer &amp; Co"));
        assert!(xml.contains("category id=\"2000\" name=\"Movies\""));
    }

    #[test]
    fn build_empty_search_response_includes_zero_response() {
        let xml = build_empty_search_response();
        assert!(xml.contains("torznab:response offset=\"0\" total=\"0\""));
    }

    #[test]
    fn build_search_response_renders_item_and_attrs() {
        let items = vec![TorznabFeedItem {
            guid: Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("valid uuid"),
            title: "Example".to_string(),
            size_bytes: Some(1024),
            published_at: Some(Utc::now()),
            categories: vec![2000],
            seeders: 12,
            leechers: 3,
            download_volume_factor: 1,
            infohash: Some("abc".to_string()),
            download_link: "/torznab/download".to_string(),
        }];
        let xml = build_search_response(&items, 0, 1).expect("search response should build");
        assert!(xml.contains("<item>"));
        assert!(xml.contains("torznab:attr name=\"seeders\" value=\"12\""));
        assert!(xml.contains("<category>2000</category>"));
        assert!(xml.contains("torznab:response offset=\"0\" total=\"1\""));
    }
}
