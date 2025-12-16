use std::path::Path;

use anyhow::anyhow;
use base64::{Engine as _, engine::general_purpose};
use revaer_api::models::{
    TorrentAction as ApiTorrentAction, TorrentCreateRequest, TorrentDetail, TorrentListResponse,
    TorrentSelectionRequest,
};
use revaer_torrent_core::{FilePriority, FilePriorityOverride};
use serde::Deserialize;
use uuid::Uuid;

use crate::cli::{
    ActionType, OutputFormat, TorrentActionArgs, TorrentAddArgs, TorrentListArgs,
    TorrentRemoveArgs, TorrentSelectArgs, TorrentStatusArgs,
};
use crate::client::{AppContext, CliError, CliResult, HEADER_API_KEY, classify_problem};
use crate::output::{render_torrent_detail, render_torrent_list};

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FilePriorityOverrideArg {
    pub index: u32,
    pub priority: FilePriority,
}

pub(crate) async fn handle_torrent_add(ctx: &AppContext, args: TorrentAddArgs) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let id = args.id.unwrap_or_else(Uuid::new_v4);
    let source = args.source.trim();
    if source.is_empty() {
        return Err(CliError::validation("source must not be empty"));
    }

    let mut request = TorrentCreateRequest {
        id,
        magnet: None,
        metainfo: None,
        name: args.name,
        download_dir: None,
        sequential: None,
        start_paused: None,
        seed_mode: None,
        hash_check_sample_pct: None,
        super_seeding: None,
        include: Vec::new(),
        exclude: Vec::new(),
        skip_fluff: false,
        tags: Vec::new(),
        trackers: Vec::new(),
        replace_trackers: false,
        web_seeds: Vec::new(),
        replace_web_seeds: false,
        max_download_bps: None,
        max_upload_bps: None,
        max_connections: None,
        seed_ratio_limit: None,
        seed_time_limit: None,
        auto_managed: None,
        queue_position: None,
        pex_enabled: None,
    };

    if source.starts_with("magnet:") {
        request.magnet = Some(source.to_string());
    } else {
        let path = Path::new(source);
        let bytes = std::fs::read(path).map_err(|err| {
            CliError::failure(anyhow!(
                "failed to read torrent file '{}': {err}",
                path.display()
            ))
        })?;
        request.metainfo = Some(general_purpose::STANDARD.encode(&bytes));
        if request.name.is_none() {
            request.name = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string);
        }
    }

    let url = ctx
        .base_url
        .join("/v1/torrents")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| CliError::failure(anyhow!("request to /v1/torrents failed: {err}")))?;

    if response.status().is_success() {
        println!("Torrent submission requested (id: {id})");
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_torrent_remove(
    ctx: &AppContext,
    args: TorrentRemoveArgs,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;
    let id = args.id;

    let url = ctx
        .base_url
        .join(&format!("/v1/torrents/{id}/action"))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&ApiTorrentAction::Remove { delete_data: false })
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!("request to /v1/torrents/{id}/action failed: {err}"))
        })?;

    if response.status().is_success() {
        println!("Torrent removal requested (id: {id})");
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_torrent_list(
    ctx: &AppContext,
    args: TorrentListArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let mut url = ctx
        .base_url
        .join("/v1/torrents")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    {
        let mut pairs = url.query_pairs_mut();
        if let Some(limit) = args.limit {
            pairs.append_pair("limit", &limit.to_string());
        }
        if let Some(cursor) = &args.cursor {
            pairs.append_pair("cursor", cursor);
        }
        if let Some(state) = &args.state {
            pairs.append_pair("state", state);
        }
        if let Some(tracker) = &args.tracker {
            pairs.append_pair("tracker", tracker);
        }
        if let Some(extension) = &args.extension {
            pairs.append_pair("extension", extension);
        }
        if let Some(tags) = &args.tags {
            pairs.append_pair("tags", tags);
        }
        if let Some(name) = &args.name {
            pairs.append_pair("name", name);
        }
    }

    let response = ctx
        .client
        .get(url)
        .send()
        .await
        .map_err(|err| CliError::failure(anyhow!("request to /v1/torrents failed: {err}")))?;

    if response.status().is_success() {
        let list = response
            .json::<TorrentListResponse>()
            .await
            .map_err(|err| CliError::failure(anyhow!("failed to parse torrent list: {err}")))?;
        render_torrent_list(&list, output)?;
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_torrent_status(
    ctx: &AppContext,
    args: TorrentStatusArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let url = ctx
        .base_url
        .join(&format!("/v1/torrents/{}", args.id))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;
    let response = ctx.client.get(url.as_ref()).send().await.map_err(|err| {
        CliError::failure(anyhow!("request to /v1/torrents/{{id}} failed: {err}"))
    })?;

    if response.status().is_success() {
        let detail = response
            .json::<TorrentDetail>()
            .await
            .map_err(|err| CliError::failure(anyhow!("failed to parse torrent detail: {err}")))?;
        render_torrent_detail(&detail, output)?;
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_torrent_select(
    ctx: &AppContext,
    args: TorrentSelectArgs,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let mut request = TorrentSelectionRequest {
        include: args.include.clone(),
        exclude: args.exclude.clone(),
        skip_fluff: Some(args.skip_fluff),
        priorities: Vec::new(),
    };
    for entry in &args.priorities {
        request.priorities.push(FilePriorityOverride {
            index: entry.index,
            priority: entry.priority,
        });
    }

    let url = ctx
        .base_url
        .join(&format!("/v1/torrents/{}/select", args.id))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/torrents/{{id}}/select failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        println!("Selection update accepted (id: {})", args.id);
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_torrent_action(
    ctx: &AppContext,
    args: TorrentActionArgs,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let action_payload = build_action_payload(&args)?;

    let url = ctx
        .base_url
        .join(&format!("/v1/torrents/{}/action", args.id))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&action_payload)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/torrents/{{id}}/action failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        println!("Action dispatched (id: {})", args.id);
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) fn build_action_payload(args: &TorrentActionArgs) -> CliResult<ApiTorrentAction> {
    let action = match args.action {
        ActionType::Pause => ApiTorrentAction::Pause,
        ActionType::Resume => ApiTorrentAction::Resume,
        ActionType::Remove => ApiTorrentAction::Remove {
            delete_data: args.delete_data,
        },
        ActionType::Reannounce => ApiTorrentAction::Reannounce,
        ActionType::Recheck => ApiTorrentAction::Recheck,
        ActionType::Sequential => {
            let enable = args.enable.ok_or_else(|| {
                CliError::validation("--enable must be provided for sequential action")
            })?;
            ApiTorrentAction::Sequential { enable }
        }
        ActionType::Rate => {
            if args.download.is_none() && args.upload.is_none() {
                return Err(CliError::validation(
                    "provide --download and/or --upload when action=rate",
                ));
            }
            ApiTorrentAction::Rate {
                download_bps: args.download,
                upload_bps: args.upload,
            }
        }
    };
    Ok(action)
}

pub(crate) fn parse_priority_override(value: &str) -> Result<FilePriorityOverrideArg, String> {
    let (index_str, priority_str) = value
        .split_once('=')
        .ok_or_else(|| "expected format index=priority".to_string())?;
    let index = index_str
        .trim()
        .parse::<u32>()
        .map_err(|_| "index must be an integer".to_string())?;
    let priority = match priority_str.trim().to_ascii_lowercase().as_str() {
        "skip" => FilePriority::Skip,
        "low" => FilePriority::Low,
        "normal" => FilePriority::Normal,
        "high" => FilePriority::High,
        other => return Err(format!("unknown priority '{other}'")),
    };
    Ok(FilePriorityOverrideArg { index, priority })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use httpmock::prelude::*;
    use reqwest::Client;
    use revaer_api::models::{
        TorrentFileView, TorrentProgressView, TorrentRatesView, TorrentStateKind, TorrentStateView,
        TorrentSummary,
    };
    use serde_json::json;

    use crate::client::{ApiKeyCredential, parse_api_key};
    use crate::output::{format_bytes, format_priority, state_to_str};

    fn context_with(server: &MockServer, api_key: Option<ApiKeyCredential>) -> AppContext {
        AppContext {
            client: Client::new(),
            base_url: server.base_url().parse().expect("valid URL"),
            api_key,
        }
    }

    fn context_with_key(server: &MockServer) -> AppContext {
        context_with(
            server,
            Some(ApiKeyCredential {
                key_id: "key".to_string(),
                secret: "secret".to_string(),
            }),
        )
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
            library_path: Some("/library/example".into()),
            download_dir: Some("/downloads/example".into()),
            sequential: false,
            tags: vec!["tag1".into()],
            trackers: vec!["https://tracker.example/announce".into()],
            rate_limit: None,
            connections_limit: None,
            added_at: now,
            completed_at: None,
            last_updated: now,
        }
    }

    #[tokio::test]
    async fn torrent_add_issues_post_request() {
        let server = MockServer::start_async().await;
        let id = Uuid::new_v4();
        let magnet = "magnet:?xt=urn:btih:demo";
        let name = "demo.torrent";

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/torrents")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "id": id,
                    "magnet": magnet,
                    "metainfo": null,
                    "name": name,
                    "download_dir": null,
                    "sequential": null,
                    "start_paused": null,
                    "seed_mode": null,
                    "hash_check_sample_pct": null,
                    "super_seeding": null,
                    "include": [],
                    "exclude": [],
                    "skip_fluff": false,
                    "tags": [],
                    "trackers": [],
                    "replace_trackers": false,
                    "web_seeds": [],
                    "replace_web_seeds": false,
                    "max_download_bps": null,
                    "max_upload_bps": null,
                    "max_connections": null,
                    "seed_ratio_limit": null,
                    "seed_time_limit": null,
                    "auto_managed": null,
                    "queue_position": null,
                    "pex_enabled": null
                }));
            then.status(202);
        });

        let ctx = context_with_key(&server);
        let args = TorrentAddArgs {
            source: magnet.to_string(),
            name: Some(name.to_string()),
            id: Some(id),
        };

        handle_torrent_add(&ctx, args)
            .await
            .expect("torrent add should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn torrent_remove_issues_delete_request() {
        let server = MockServer::start_async().await;
        let id = Uuid::new_v4();

        let path = format!("/v1/torrents/{id}/action");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "type": "remove",
                    "delete_data": false
                }));
            then.status(202);
        });

        let ctx = context_with_key(&server);
        let args = TorrentRemoveArgs { id };

        handle_torrent_remove(&ctx, args)
            .await
            .expect("torrent remove should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn torrent_list_renders_table() {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let now = Utc::now();
        let list = TorrentListResponse {
            torrents: vec![sample_summary(torrent_id, now)],
            next: Some("cursor-1".into()),
        };
        server.mock(move |when, then| {
            when.method(GET).path("/v1/torrents");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!(list));
        });

        let ctx = context_with(&server, None);
        handle_torrent_list(&ctx, TorrentListArgs::default(), OutputFormat::Table)
            .await
            .expect("list should succeed");
    }

    #[tokio::test]
    async fn torrent_status_renders_detail() {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let now = Utc::now();
        let detail = TorrentDetail {
            summary: sample_summary(torrent_id, now),
            settings: None,
            files: Some(vec![TorrentFileView {
                index: 0,
                path: "example.mkv".into(),
                size_bytes: 2_048,
                bytes_completed: 1_024,
                priority: FilePriority::High,
                selected: true,
            }]),
        };
        server.mock(move |when, then| {
            when.method(GET)
                .path(format!("/v1/torrents/{torrent_id}").as_str());
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!(detail));
        });

        let ctx = context_with(&server, None);
        handle_torrent_status(
            &ctx,
            TorrentStatusArgs { id: torrent_id },
            OutputFormat::Table,
        )
        .await
        .expect("status should succeed");
    }

    #[tokio::test]
    async fn torrent_select_sends_priorities() {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let path = format!("/v1/torrents/{torrent_id}/select");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret");
            then.status(200);
        });

        let ctx = context_with_key(&server);
        let args = TorrentSelectArgs {
            id: torrent_id,
            include: vec!["**/*.mkv".into()],
            exclude: Vec::new(),
            skip_fluff: true,
            priorities: vec![FilePriorityOverrideArg {
                index: 7,
                priority: FilePriority::High,
            }],
        };
        handle_torrent_select(&ctx, args)
            .await
            .expect("select should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn torrent_action_sequential_with_enable() {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let path = format!("/v1/torrents/{torrent_id}/action");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "type": "sequential",
                    "enable": true
                }));
            then.status(202);
        });

        let ctx = context_with_key(&server);
        let args = TorrentActionArgs {
            id: torrent_id,
            action: ActionType::Sequential,
            enable: Some(true),
            delete_data: false,
            download: None,
            upload: None,
        };

        handle_torrent_action(&ctx, args)
            .await
            .expect("action should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn torrent_action_rate_includes_caps() {
        let server = MockServer::start_async().await;
        let torrent_id = Uuid::new_v4();
        let path = format!("/v1/torrents/{torrent_id}/action");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "type": "rate",
                    "download_bps": 2048,
                    "upload_bps": 1024
                }));
            then.status(202);
        });

        let ctx = context_with_key(&server);
        let args = TorrentActionArgs {
            id: torrent_id,
            action: ActionType::Rate,
            enable: None,
            delete_data: false,
            download: Some(2048),
            upload: Some(1024),
        };

        handle_torrent_action(&ctx, args)
            .await
            .expect("rate action should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn stream_events_discards_malformed_payloads() {
        let server = MockServer::start_async().await;
        server.mock(|when, then| {
            when.method(GET).path("/v1/torrents/events");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body("id:2\ndata:{\"bad\":true}\n\n");
        });

        let ctx = context_with_key(&server);
        let args = crate::cli::TailArgs {
            torrent: Vec::new(),
            event: Vec::new(),
            state: Vec::new(),
            resume_file: None,
            retry_secs: 0,
        };
        let response = ctx
            .client
            .get(ctx.base_url.join("/v1/torrents/events").unwrap())
            .send()
            .await
            .expect("send request");
        let id = crate::commands::tail::stream_events(response, &args, None)
            .await
            .expect("stream should succeed");
        assert_eq!(id, Some(2));
    }

    #[test]
    fn parse_priority_override_rejects_invalid_payload() {
        let err = parse_priority_override("abc=skip").expect_err("invalid index should fail");
        assert!(err.contains("index"));
        let err = parse_priority_override("10=unknown").expect_err("invalid priority");
        assert!(err.contains("unknown priority"));
    }

    #[test]
    fn parse_priority_override_accepts_values() {
        let parsed = parse_priority_override("42=high").expect("valid override");
        assert_eq!(parsed.index, 42);
        assert_eq!(parsed.priority, FilePriority::High);
    }

    #[test]
    fn format_bytes_displays_expected_units() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.00 KiB");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.00 MiB");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.00 GiB");
    }

    #[test]
    fn state_to_str_maps_variants() {
        assert_eq!(state_to_str(TorrentStateKind::Queued), "queued");
        assert_eq!(state_to_str(TorrentStateKind::Completed), "completed");
    }

    #[test]
    fn format_priority_labels_variants() {
        assert_eq!(format_priority(FilePriority::Skip), "skip");
        assert_eq!(format_priority(FilePriority::High), "high");
    }

    #[test]
    fn build_action_payload_requires_enable_flag() {
        let args = TorrentActionArgs {
            id: Uuid::new_v4(),
            action: ActionType::Sequential,
            enable: None,
            delete_data: false,
            download: None,
            upload: None,
        };
        let err = build_action_payload(&args).expect_err("missing enable should fail");
        assert!(matches!(err, CliError::Validation(message) if message.contains("--enable")));
    }

    #[test]
    fn build_action_payload_validates_rate_limits() {
        let args = TorrentActionArgs {
            id: Uuid::new_v4(),
            action: ActionType::Rate,
            enable: None,
            delete_data: false,
            download: None,
            upload: None,
        };
        let err =
            build_action_payload(&args).expect_err("missing rate values should fail validation");
        assert!(matches!(err, CliError::Validation(message) if message.contains("download")));
    }

    #[test]
    fn build_action_payload_rate_accepts_partial_limits() -> CliResult<()> {
        let args = TorrentActionArgs {
            id: Uuid::new_v4(),
            action: ActionType::Rate,
            enable: None,
            delete_data: false,
            download: Some(1024),
            upload: None,
        };
        match build_action_payload(&args)? {
            ApiTorrentAction::Rate {
                download_bps,
                upload_bps,
            } => {
                assert_eq!(download_bps, Some(1024));
                assert_eq!(upload_bps, None);
            }
            other => panic!("unexpected payload {other:?}"),
        }
        Ok(())
    }

    #[tokio::test]
    async fn parse_api_key_accepts_valid_pair() -> CliResult<()> {
        let parsed = parse_api_key(Some("alpha:bravo".to_string()))?.expect("expected credentials");
        assert_eq!(parsed.key_id, "alpha");
        assert_eq!(parsed.secret, "bravo");
        Ok(())
    }
}
