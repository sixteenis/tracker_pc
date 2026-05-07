//! 3분 주기 heartbeat (기획서 §20). `can_track_time = false` 면 전송 생략.
//! 서버 응답의 `next_heartbeat_seconds` 로 다음 주기를 동적으로 조정.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tracing::warn;

use crate::api::types::HeartbeatRequest;
use crate::app::{AppState, PcStatus};
use crate::db::settings_repo;

const KEY_LAST_HEARTBEAT: &str = "last_heartbeat_at";

pub async fn run(state: Arc<AppState>) {
    let mut interval_secs = state.config.intervals.heartbeat_interval_seconds.max(30);

    loop {
        tokio::time::sleep(Duration::from_secs(interval_secs)).await;

        let maybe_session = state.session.read().unwrap().clone();
        let session = match maybe_session {
            Some(s) => s,
            None => continue,
        };
        let snapshot = state.snapshot_status();
        if !snapshot.can_track_time {
            // 요금제 미포함 — heartbeat 도 최소화 (skip).
            continue;
        }

        let beat = HeartbeatRequest {
            company_id: session.company_id.clone(),
            employee_id: session.employee_id.clone(),
            device_id: state.device.device_id.clone(),
            device_name: state.device.device_name.clone(),
            app_version: state.config.app.app_version.clone(),
            pc_status: snapshot.pc_status.as_str().to_string(),
            last_activity_at: snapshot.last_activity_at,
            idle_seconds: snapshot.idle_seconds,
            is_locked: snapshot.is_locked,
            attendance_status: snapshot.attendance,
            can_track_time: snapshot.can_track_time,
            effective_idle_threshold_seconds: snapshot.effective_idle_threshold_seconds,
        };

        match state.api.send_heartbeat(&session.access_token, beat).await {
            Ok(resp) => {
                interval_secs = resp.next_heartbeat_seconds.clamp(30, 1800);
                if !resp.can_track_time {
                    if let Ok(mut s) = state.status.write() {
                        s.can_track_time = false;
                        s.pc_status = PcStatus::Offline;
                    }
                }
                let now = Utc::now();
                if let Ok(mut s) = state.status.write() {
                    s.last_heartbeat_at = Some(now);
                }
                let _ = settings_repo::set(&state.db, KEY_LAST_HEARTBEAT, &now.to_rfc3339());

                if resp.force_logout {
                    warn!("서버가 강제 로그아웃을 요청 — 세션 해제");
                    let _ = crate::auth::logout(&state);
                }
            }
            Err(e) => {
                warn!(error = %e, "heartbeat 실패");
            }
        }
    }
}
