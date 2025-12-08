use std::fs;
use std::time::Duration;

use anyhow::anyhow;
use futures_util::StreamExt;
use revaer_events::EventEnvelope;
use uuid::Uuid;

use crate::cli::TailArgs;
use crate::client::{
    AppContext, CliError, CliResult, HEADER_LAST_EVENT_ID, HEADER_REQUEST_ID, classify_problem,
};

pub(crate) async fn handle_tail(ctx: &AppContext, args: TailArgs) -> CliResult<()> {
    let mut resume_id = args
        .resume_file
        .as_ref()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|value| value.trim().parse::<u64>().ok());

    loop {
        let mut url = ctx
            .base_url
            .join("/v1/torrents/events")
            .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

        {
            let mut pairs = url.query_pairs_mut();
            if !args.torrent.is_empty() {
                let value = args
                    .torrent
                    .iter()
                    .map(Uuid::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                pairs.append_pair("torrent", &value);
            }
            if !args.event.is_empty() {
                let value = args.event.join(",");
                pairs.append_pair("event", &value);
            }
            if !args.state.is_empty() {
                let value = args.state.join(",");
                pairs.append_pair("state", &value);
            }
        }

        let builder = ctx.client.get(url);
        let builder = if let Some(id) = resume_id {
            builder.header(HEADER_LAST_EVENT_ID, id.to_string())
        } else {
            builder
        };

        let response = match builder.send().await {
            Ok(resp) => resp,
            Err(err) => {
                eprintln!(
                    "stream connection failed: {err:?}. retrying in {}s",
                    args.retry_secs
                );
                tokio::time::sleep(Duration::from_secs(args.retry_secs)).await;
                continue;
            }
        };

        if !response.status().is_success() {
            return Err(classify_problem(response).await);
        }

        match stream_events(response, &args, resume_id.as_mut()).await {
            Ok(last_id) => resume_id = last_id,
            Err(err) => {
                eprintln!("stream error: {err:?}. retrying in {}s", args.retry_secs);
                tokio::time::sleep(Duration::from_secs(args.retry_secs)).await;
            }
        }
    }
}

pub(crate) async fn stream_events(
    response: reqwest::Response,
    args: &TailArgs,
    mut resume_slot: Option<&mut u64>,
) -> CliResult<Option<u64>> {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut current_event_id: Option<u64> = None;
    let mut current_data = Vec::new();
    let mut last_seen = resume_slot.as_ref().map(|slot| **slot);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|err| CliError::failure(anyhow!("failed to read event stream: {err}")))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim_end_matches('\r').to_string();
            buffer.drain(..=pos);
            if line.is_empty() {
                if current_data.is_empty() {
                    current_event_id = None;
                    continue;
                }
                let payload = current_data.join("\n");
                current_data.clear();
                if let Some(id) = current_event_id.take() {
                    if Some(id) == last_seen {
                        continue;
                    }
                    last_seen = Some(id);
                    if let Some(slot) = resume_slot.as_mut() {
                        **slot = id;
                    }
                    if let Some(path) = &args.resume_file {
                        let _ = fs::write(path, id.to_string());
                    }
                }
                match serde_json::from_str::<EventEnvelope>(&payload) {
                    Ok(event) => {
                        let text = serde_json::to_string_pretty(&event).map_err(|err| {
                            CliError::failure(anyhow!("failed to format event JSON: {err}"))
                        })?;
                        println!("{text}");
                    }
                    Err(err) => {
                        eprintln!("discarding malformed event payload: {err} -- {payload}");
                    }
                }
            } else if let Some(data) = line.strip_prefix("data:") {
                current_data.push(data.trim_start().to_string());
            } else if let Some(id) = line.strip_prefix("id:")
                && let Ok(value) = id.trim_start().parse::<u64>()
            {
                current_event_id = Some(value);
            } else if line.starts_with("event:")
                || line.starts_with("retry:")
                || line.starts_with(HEADER_REQUEST_ID)
            {
                // ignore auxiliary fields
            }
        }
    }

    Ok(last_seen)
}
