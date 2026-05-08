//! ============================================================================
//! sync::heartbeat — 3분 주기 PC 상태 보고 (기획서 §20).
//! ============================================================================
//!
//! - `can_track_time = false` 면 전송 생략 (요금제 미포함 시 서버 부하 절약).
//! - 서버 응답의 `next_heartbeat_seconds` 로 다음 주기를 동적으로 조정 (30~1800 클램프).
//! - 응답에 `force_logout = true` 가 오면 즉시 `auth::logout` 호출.
//!
//! TODO(서버 연동): heartbeat 응답에 `attendance_status` 도 포함시키면 별도
//! attendance_sync 폴링이 불필요해짐 (한 번의 RPC 로 모든 상태 동기화).
//! TODO(UI 갱신): force_logout 호출 시 UI 가 자동으로 로그인 화면으로 가지 않음.
//! `auth::logout` 이 단순히 세션만 비울 뿐, 라우터에는 별도 신호가 없음. 다음 UI
//! 프레임에서 `state.is_logged_in() == false` 를 보고 자동 분기되긴 하지만
//! 안내 토스트는 미구현.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tracing::warn;

use crate::data::dto::HeartbeatRequest;
use crate::app::{AppState, PcStatus};
use crate::data::local::settings_repo;

const KEY_LAST_HEARTBEAT: &str = "last_heartbeat_at";

/// 메인 heartbeat 루프. 앱 종료까지 무한 반복.
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
            company_id: session.company_id_str.clone(),
            employee_id: session.employee_id_str.clone(),
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

        match state.api.send_heartbeat(beat).await {
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
                    let _ = crate::domain::service::user_service::logout(&state);
                }
            }
            Err(e) => {
                warn!(error = %e, "heartbeat 실패");
            }
        }
    }
}
