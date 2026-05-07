//! 앱 시작/종료 이벤트 + 비정상 종료 후 재시작 시 NO_PC_RECORD 구간 생성.
//!
//! 동작:
//! - `record_started` : APP_STARTED 이벤트 enqueue. 이전 heartbeat 가
//!   `settings.last_heartbeat_at` 에 저장돼 있고, 그 시간이 정책 임계값보다
//!   더 과거라면 NO_PC_RECORD segment 를 추가한다.
//! - `record_stopped` : APP_STOPPED 이벤트 enqueue. (메인 루프 종료 직전 호출)

use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::app::AppState;
use crate::db::events_repo;
use crate::db::idle_segments_repo::{self, NewSegment, SegmentType};
use crate::db::settings_repo;

const KEY_LAST_HEARTBEAT: &str = "last_heartbeat_at";

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
                    let _ = idle_segments_repo::insert(
                        &state.db,
                        &NewSegment {
                            company_id: session.company_id,
                            employee_id: session.employee_id,
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
                                now + chrono::Duration::hours(48),
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
