//! 1분 주기 이벤트 배치 전송 (기획서 §17, §18, §22).

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tracing::{info, warn};

use crate::api::types::{EventEntry, EventsBatch};
use crate::app::AppState;
use crate::db::events_repo;

pub async fn run(state: Arc<AppState>) {
    let interval = Duration::from_secs(
        state.config.intervals.event_batch_interval_seconds.max(15),
    );
    let limit = state.config.intervals.max_events_per_batch.max(1);

    loop {
        tokio::time::sleep(interval).await;

        let maybe_session = state.session.read().unwrap().clone();
        let session = match maybe_session {
            Some(s) => s,
            None => continue,
        };
        if !state.can_track_time() {
            continue;
        }

        let pending = match events_repo::pending_batch(&state.db, limit) {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "PENDING 이벤트 조회 실패");
                continue;
            }
        };
        if pending.is_empty() {
            continue;
        }

        let event_ids: Vec<String> = pending.iter().map(|e| e.event_id.clone()).collect();
        let entries: Vec<EventEntry> = pending
            .into_iter()
            .map(|e| EventEntry {
                event_id: e.event_id,
                event_type: e.event_type,
                event_time: e.event_time,
                payload: serde_json::from_str(&e.payload_json)
                    .unwrap_or(serde_json::Value::Null),
            })
            .collect();

        let batch = EventsBatch {
            company_id: session.company_id.clone(),
            employee_id: session.employee_id.clone(),
            device_id: state.device.device_id.clone(),
            events: entries,
        };

        match state.api.send_events(&session.access_token, batch).await {
            Ok(resp) => {
                let count = resp.accepted_event_ids.len();
                let _ = events_repo::mark_success(&state.db, &resp.accepted_event_ids);
                if let Ok(mut s) = state.status.write() {
                    s.last_event_sync_at = Some(Utc::now());
                }
                info!(count, "이벤트 배치 전송 성공");
            }
            Err(e) => {
                warn!(error = %e, count = event_ids.len(), "이벤트 배치 전송 실패 — 재시도 대기");
                let _ = events_repo::mark_failed(&state.db, &event_ids, &e.to_string());
            }
        }
    }
}
