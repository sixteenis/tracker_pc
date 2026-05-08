//! ============================================================================
//! monitor::lifecycle — 앱 시작/종료 이벤트 + NO_PC_RECORD 자동 생성.
//! ============================================================================
//!
//! - `record_started` : `APP_STARTED` 이벤트 enqueue. 이전 heartbeat 가
//!   `settings.last_heartbeat_at` 에 저장돼 있고 그 시각이 정책 임계값보다 더
//!   과거라면 `NO_PC_RECORD` segment 를 추가 (앱 비정상 종료 / PC 종료 흔적).
//! - `record_stopped` : `APP_STOPPED` 이벤트 enqueue. (메인 루프 종료 직전 호출 예정)
//!
//! TODO(미연결): `record_stopped` 가 현재 어디에서도 호출되지 않음.
//! `eframe::App::on_exit` 또는 `Drop for AppState` 에 hook 추가 필요.
//! TODO(2차): `record_started` 에서 NO_PC_RECORD 만들 때 출근 상태 (`attendance_status`)
//! 가 WORKING 인 시간대만 잡도록 — 퇴근 후 PC 종료는 정상이므로 segment 불필요.

use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::app::AppState;
use crate::data::local::events_repo;
use crate::data::local::idle_segments_repo::{self, NewSegment, SegmentType};
use crate::data::local::settings_repo;

const KEY_LAST_HEARTBEAT: &str = "last_heartbeat_at";

/// 앱 시작 직후 호출 (`monitor::spawn_all` 에서). APP_STARTED 이벤트 + 필요 시
/// NO_PC_RECORD segment 생성. 세션이 아직 없으면 NO_PC_RECORD 는 생략됨
/// (자동로그인 성공 후 상태가 안정되어야 의미 있는 segment 가 됨).
pub fn record_started(state: &Arc<AppState>) -> Result<()> {
    let now = Utc::now();
    events_repo::enqueue(
        &state.db,
        "APP_STARTED",
        now,
        &serde_json::json!({
            "app_version": state.config.app.app_version,
            "device_id": state.device.device_id,
            "device_name": state.device.device_name,
        }),
    )?;

    // 기록 없음 구간 검사
    if let Some(last_str) = settings_repo::get(&state.db, KEY_LAST_HEARTBEAT)? {
        if let Ok(last) = DateTime::parse_from_rfc3339(&last_str) {
            let last = last.with_timezone(&Utc);
            let gap = (now - last).num_seconds();
            let threshold = state
                .status
                .read()
                .map(|s| s.effective_idle_threshold_seconds)
                .unwrap_or(600) as i64;
            if gap > threshold {
                if let Some(session) = state.session.read().unwrap().clone() {
                    let deadline_hours = state.snapshot_policy().explanation_deadline_hours.max(1);
                    let _ = idle_segments_repo::insert(
                        &state.db,
                        &NewSegment {
                            company_id: session.company_id_str.clone(),
                            employee_id: session.employee_id_str.clone(),
                            device_id: state.device.device_id.clone(),
                            work_date: now.date_naive(),
                            segment_type: SegmentType::NoPcRecord,
                            start_time: last,
                            end_time: Some(now),
                            applied_idle_threshold_seconds: threshold,
                            policy_scope: state
                                .status
                                .read()
                                .map(|s| s.policy_scope.clone())
                                .unwrap_or_default(),
                            explanation_deadline: Some(
                                now + chrono::Duration::hours(deadline_hours as i64),
                            ),
                        },
                    );
                    events_repo::enqueue(
                        &state.db,
                        "NO_PC_RECORD",
                        now,
                        &serde_json::json!({
                            "from": last.to_rfc3339(),
                            "to": now.to_rfc3339(),
                            "duration_seconds": gap,
                        }),
                    )?;
                }
            }
        }
    }
    Ok(())
}

/// 앱 정상 종료 직전 호출 — APP_STOPPED 이벤트 enqueue.
/// TODO(미연결): 호출 hook 부재. main.rs 의 `eframe::run_native` 가 반환된 직후
/// `info!("핀플 PC 앱 종료")` 직전에 호출하는 것이 자연스러움.
#[allow(dead_code)]
pub fn record_stopped(state: &Arc<AppState>) -> Result<()> {
    events_repo::enqueue(
        &state.db,
        "APP_STOPPED",
        Utc::now(),
        &serde_json::json!({ "device_id": state.device.device_id }),
    )?;
    Ok(())
}
