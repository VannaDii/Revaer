//! Filesystem post-processing placeholder crate.

use anyhow::{Result, ensure};
use revaer_config::FsPolicy;
use revaer_events::{Event, EventBus};
use tracing::info;
use uuid::Uuid;

/// Service responsible for executing filesystem post-processing steps after torrent completion.
#[derive(Clone)]
pub struct FsOpsService {
    events: EventBus,
}

impl FsOpsService {
    /// Construct a new filesystem operations service backed by the shared event bus.
    #[must_use]
    pub fn new(events: EventBus) -> Self {
        Self { events }
    }

    /// Apply the configured filesystem policy for the given torrent and emit progress events.
    ///
    /// # Errors
    ///
    /// Returns an error if any filesystem post-processing step fails.
    pub fn apply_policy(&self, torrent_id: Uuid, policy: &FsPolicy) -> Result<()> {
        let _ = self.events.publish(Event::FsopsStarted { torrent_id });
        let _ = self.events.publish(Event::FsopsProgress {
            torrent_id,
            step: "applying_policy".to_string(),
        });

        let result = (|| -> Result<()> {
            ensure!(
                !policy.library_root.trim().is_empty(),
                "filesystem policy library root cannot be empty"
            );
            info!("Applying filesystem policy at {}", policy.library_root);
            Ok(())
        })();

        match &result {
            Ok(()) => {
                let _ = self.events.publish(Event::FsopsCompleted { torrent_id });
            }
            Err(error) => {
                let message = format!("{error:#}");
                let _ = self.events.publish(Event::FsopsFailed {
                    torrent_id,
                    message,
                });
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revaer_events::{Event, EventBus};
    use serde_json::json;
    use tokio::time::{Duration, timeout};

    fn policy(root: &str) -> FsPolicy {
        FsPolicy {
            id: Uuid::new_v4(),
            library_root: root.to_string(),
            extract: false,
            par2: "disabled".to_string(),
            flatten: false,
            move_mode: "copy".to_string(),
            cleanup_keep: json!([]),
            cleanup_drop: json!([]),
            chmod_file: None,
            chmod_dir: None,
            owner: None,
            group: None,
            umask: None,
            allow_paths: json!([]),
        }
    }

    async fn next_event(stream: &mut revaer_events::EventStream) -> Event {
        timeout(Duration::from_millis(100), stream.next())
            .await
            .expect("timed out waiting for event")
            .expect("event stream closed unexpectedly")
            .event
    }

    #[tokio::test]
    async fn apply_policy_emits_successful_lifecycle() -> Result<()> {
        let bus = EventBus::with_capacity(8);
        let service = FsOpsService::new(bus.clone());
        let mut stream = bus.subscribe(None);
        let torrent_id = Uuid::new_v4();

        service.apply_policy(torrent_id, &policy("/media/library"))?;

        let started = next_event(&mut stream).await;
        assert!(matches!(
            started,
            Event::FsopsStarted { torrent_id: id } if id == torrent_id
        ));

        let progress = next_event(&mut stream).await;
        assert!(matches!(
            progress,
            Event::FsopsProgress { torrent_id: id, step } if id == torrent_id && step == "applying_policy"
        ));

        let completed = next_event(&mut stream).await;
        assert!(matches!(
            completed,
            Event::FsopsCompleted { torrent_id: id } if id == torrent_id
        ));

        Ok(())
    }

    #[tokio::test]
    async fn apply_policy_failure_emits_failed_event() {
        let bus = EventBus::with_capacity(8);
        let service = FsOpsService::new(bus.clone());
        let mut stream = bus.subscribe(None);
        let torrent_id = Uuid::new_v4();

        let result = service.apply_policy(torrent_id, &policy("  "));
        assert!(
            result.is_err(),
            "expected validation failure for empty root"
        );

        let mut saw_failed = false;
        for _ in 0..3 {
            if let Event::FsopsFailed {
                torrent_id: id,
                message,
            } = next_event(&mut stream).await
            {
                assert_eq!(id, torrent_id);
                assert!(message.contains("cannot be empty"));
                saw_failed = true;
                break;
            }
        }

        assert!(
            saw_failed,
            "expected FsopsFailed event after validation error"
        );
    }
}
